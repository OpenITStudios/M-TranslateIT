use std::fs;
use std::io;
use std::path::Path;

// A fresh guided Stop owns the Draft paths only when neither existed before
// writing. An orphaned review from a prior interrupted call may be the only
// recoverable audio: never overwrite it or delete it as error cleanup.
fn require_unowned_draft_absent(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err("VoiceLab found existing Draft audio without an active review owner. The existing recording was preserved instead of being overwritten.".to_string()),
        Err(_) => Err("VoiceLab could not verify ownership of existing Draft audio. No recording was overwritten.".to_string()),
    }
}

// This is the only publish path for a fresh unaccepted guided recording.
// The app-level PendingDraft mutex spans the pair write and registration.
// The writer callback is injectable solely to exercise individual I/O failure
// ordering in isolated Rust fixtures; production uses canonical PCM16 writer.
pub(super) fn write_guided_review_draft_pair(
    review_path: &Path,
    native_path: &Path,
    write_native: impl FnOnce(&Path) -> io::Result<()>,
    write_review: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<(), String> {
    if review_path == native_path {
        return Err("VoiceLab review and native-rate Draft paths cannot be identical.".to_string());
    }
    require_unowned_draft_absent(review_path)?;
    require_unowned_draft_absent(native_path)?;

    // First write succeeds => this invocation, not a previous recording,
    // created native_path. First-write failures must not delete either path.
    write_native(native_path)
        .map_err(|error| format!("VoiceLab could not save the native-rate draft: {error}. Existing Draft audio was not removed."))?;

    if let Err(error) = write_review(review_path) {
        // The canonical WAV writer either publishes the complete review WAV
        // or returns error without replacing any valid destination. Delete
        // only the native draft whose successful write we own here.
        match fs::remove_file(native_path) {
            Ok(()) => Err(format!("VoiceLab could not save the review draft: {error}. This attempt's native-rate draft was rolled back.")),
            Err(cleanup) if cleanup.kind() == io::ErrorKind::NotFound => {
                Err(format!("VoiceLab could not save the review draft: {error}. No native-rate draft remains."))
            }
            Err(cleanup) => Err(format!(
                "VoiceLab could not save the review draft: {error}. Native-rate rollback could not be verified: {cleanup}. Existing files require recovery before another attempt."
            )),
        }
    } else {
        Ok(())
    }
}

pub(super) fn discard_review_take(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path)
        .map_err(|error| format!("VoiceLab could not remove the review take safely: {error}"))
}

pub(super) fn accept_review_take(draft: &Path, target: &Path) -> Result<(), String> {
    let previous = target.with_extension("wav.previous");

    if previous.exists() && !target.exists() {
        fs::rename(&previous, target).map_err(|error| {
            format!(
                "VoiceLab found an interrupted previous-take backup but could not restore it safely: {error}"
            )
        })?;
    }

    if previous.exists() && target.exists() {
        fs::remove_file(&previous).map_err(|error| {
            format!("VoiceLab could not clear the stale previous-take backup safely: {error}")
        })?;
    }

    if target.exists() {
        fs::rename(target, &previous).map_err(|error| {
            format!("VoiceLab could not preserve the previous accepted take: {error}")
        })?;
    }

    if let Err(error) = fs::rename(draft, target) {
        if previous.exists() && !target.exists() {
            if let Err(rollback_error) = fs::rename(&previous, target) {
                return Err(format!(
                    "VoiceLab could not accept this take: {error}; previous accepted take rollback also failed: {rollback_error}"
                ));
            }
        }
        return Err(format!("VoiceLab could not accept this take: {error}"));
    }

    if previous.exists() {
        let _ = fs::remove_file(previous);
    }
    Ok(())
}


/// Preserve a device-native-rate mono sidecar alongside the canonical 32 kHz
/// take. GPT-SoVITS continues to consume ONLY the canonical take. This helper
/// rolls back ordinary file-operation failures; interrupted power-loss pairs
/// must be integrity-checked before any future enhancement reads the sidecar.
pub(super) fn accept_review_take_with_native_rate(
    draft: &Path,
    target: &Path,
    native_draft: &Path,
    native_target: &Path,
) -> Result<(), String> {
    if !draft.is_file() || !native_draft.is_file() {
        return Err("VoiceLab cannot accept the take without both review and native-rate drafts.".to_string());
    }
    let parent = native_target.parent().ok_or("VoiceLab native-rate take directory is invalid.")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("VoiceLab could not prepare native-rate take storage: {error}"))?;

    let previous = native_target.with_extension("wav.previous");
    if previous.exists() && !native_target.exists() {
        fs::rename(&previous, native_target).map_err(|error| {
            format!("VoiceLab could not recover an interrupted native-rate backup: {error}")
        })?;
    }
    // Never delete an ambiguous backup: it may be the only remaining source
    // after an interrupted accept. Canonical training remains unaffected.
    if previous.exists() {
        return Err("VoiceLab native-rate backup needs recovery before replacing this take.".to_string());
    }
    if native_target.exists() && !native_target.is_file() {
        return Err("VoiceLab native-rate destination is not a file.".to_string());
    }

    let had_previous = native_target.is_file();
    if had_previous {
        fs::rename(native_target, &previous).map_err(|error| {
            format!("VoiceLab could not preserve the previous native-rate take: {error}")
        })?;
    }
    if let Err(error) = fs::rename(native_draft, native_target) {
        if had_previous {
            fs::rename(&previous, native_target).map_err(|restore| {
                format!(
                    "VoiceLab could not save the native-rate take: {error}; previous native-rate rollback failed: {restore}"
                )
            })?;
        }
        return Err(format!("VoiceLab could not save the native-rate take: {error}"));
    }
    if let Err(error) = accept_review_take(draft, target) {
        let draft_restored = fs::rename(native_target, native_draft);
        let previous_restored = if had_previous {
            fs::rename(&previous, native_target)
        } else {
            Ok(())
        };
        if draft_restored.is_err() || previous_restored.is_err() {
            return Err(format!(
                "{error}; native-rate rollback incomplete (draft={}, previous={}).",
                draft_restored.is_ok(),
                previous_restored.is_ok(),
            ));
        }
        return Err(error);
    }
    if had_previous {
        // Same best-effort stale-backup cleanup as the canonical take owner.
        // The new canonical/native pair has already been accepted.
        let _ = fs::remove_file(previous);
    }
    Ok(())
}

#[cfg(test)]
mod native_rate_take_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static RUN: AtomicU64 = AtomicU64::new(0);

    fn fixture(label: &str) -> std::path::PathBuf {
        let token = RUN.fetch_add(1, Ordering::Relaxed);
        let folder = std::env::temp_dir().join(format!(
            "translateit_native_take_{}_{}_{}",
            label,
            std::process::id(),
            token
        ));
        fs::create_dir_all(&folder).expect("create fixture");
        folder
    }

    #[test]
    fn fresh_review_pair_publishes_both_owned_wavs() {
        let root = fixture("draft_pair");
        let review = root.join("Draft").join("take.wav");
        let native = root.join("Draft").join("NativeRate").join("take.wav");
        write_guided_review_draft_pair(
            &review, &native,
            |path| { fs::create_dir_all(path.parent().unwrap())?; fs::write(path, b"native") },
            |path| { fs::create_dir_all(path.parent().unwrap())?; fs::write(path, b"review") },
        ).expect("complete new draft pair");
        assert_eq!(fs::read(&native).unwrap(), b"native");
        assert_eq!(fs::read(&review).unwrap(), b"review");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_draft_is_preserved_before_any_write() {
        let root = fixture("prior_draft");
        let review = root.join("take.wav");
        let native = root.join("NativeRate").join("take.wav");
        fs::write(&review, b"previous valid review").unwrap();
        let mut attempted = false;
        let error = write_guided_review_draft_pair(
            &review, &native,
            |_| { attempted = true; Err(io::Error::from(io::ErrorKind::PermissionDenied)) },
            |_| panic!("review writer cannot run when old Draft audio already exists"),
        ).unwrap_err();
        assert!(error.contains("preserved"));
        assert!(!attempted, "pre-existing Draft ownership must reject before either write");
        assert_eq!(fs::read(&review).unwrap(), b"previous valid review");
        assert!(!native.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_native_rate_draft_is_not_overwritten_or_removed() {
        let root = fixture("existing_native");
        let review = root.join("take.wav");
        let native = root.join("NativeRate").join("take.wav");
        fs::create_dir_all(native.parent().unwrap()).unwrap();
        fs::write(&native, b"previous valid native rate").unwrap();
        let error = write_guided_review_draft_pair(
            &review, &native,
            |_| panic!("old native rate prevents new write"),
            |_| panic!("review write must not run with unowned native draft"),
        ).unwrap_err();
        assert!(error.contains("preserved"));
        assert_eq!(fs::read(&native).unwrap(), b"previous valid native rate");
        assert!(!review.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_first_write_does_not_delete_or_publish_either_draft() {
        let root = fixture("first_native_failure");
        let review = root.join("take.wav");
        let native = root.join("NativeRate").join("take.wav");
        let error = write_guided_review_draft_pair(
            &review, &native,
            |_| Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            |_| panic!("review cannot be saved when native capture failed"),
        ).unwrap_err();
        assert!(error.contains("native-rate draft"));
        assert!(!review.exists());
        assert!(!native.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_second_draft_write_rolls_back_only_new_native_file() {
        let root = fixture("failed_review");
        let review = root.join("Draft").join("take.wav");
        let native = root.join("Draft").join("NativeRate").join("take.wav");
        let accepted = root.join("Accepted").join("take.wav");
        fs::create_dir_all(accepted.parent().unwrap()).unwrap();
        fs::write(&accepted, b"previous accepted source").unwrap();
        let error = write_guided_review_draft_pair(
            &review, &native,
            |path| { fs::create_dir_all(path.parent().unwrap())?; fs::write(path, b"owned native") },
            |_| Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ).unwrap_err();
        assert!(error.contains("rolled back"));
        assert!(!native.exists(), "only the new native artifact may be removed");
        assert!(!review.exists(), "failed second write must not claim review audio");
        assert_eq!(fs::read(&accepted).unwrap(), b"previous accepted source");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_native_rollback_is_reported_and_file_is_preserved() {
        let root = fixture("rollback_unverified");
        let review = root.join("draft.wav");
        let native = root.join("NativeRate").join("draft.wav");
        let error = write_guided_review_draft_pair(
            &review, &native,
            |path| fs::create_dir_all(path),
            |_| Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ).unwrap_err();
        assert!(error.contains("rollback could not be verified"));
        assert!(native.is_dir(), "unverified cleanup must preserve the blocking owner");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accepts_both_sources_and_replaces_a_previous_pair() {
        let root = fixture("accept");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("NativeRate").join("take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&native_draft, b"new native").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        fs::create_dir_all(native_target.parent().unwrap()).unwrap();
        fs::write(&native_target, b"old native").unwrap();
        accept_review_take_with_native_rate(&draft, &target, &native_draft, &native_target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new canonical");
        assert_eq!(fs::read(&native_target).unwrap(), b"new native");
        assert!(!native_target.with_extension("wav.previous").exists());
        assert!(!draft.exists());
        assert!(!native_draft.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_native_draft_without_changing_accepted_take() {
        let root = fixture("missing");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("NativeRate").join("take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        assert!(accept_review_take_with_native_rate(
            &draft, &target, &native_draft, &native_target
        ).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old canonical");
        assert!(draft.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failure_of_canonical_accept_rolls_back_both_native_files() {
        let root = fixture("rollback");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("NativeRate").join("take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&native_draft, b"new native").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        fs::create_dir_all(native_target.parent().unwrap()).unwrap();
        fs::write(&native_target, b"old native").unwrap();
        // A directory at the canonical rollback-file path makes the existing
        // atomic canonical accept fail after staging the native replacement.
        fs::create_dir(target.with_extension("wav.previous")).unwrap();
        assert!(accept_review_take_with_native_rate(
            &draft, &target, &native_draft, &native_target
        ).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old canonical");
        assert_eq!(fs::read(&native_target).unwrap(), b"old native");
        assert_eq!(fs::read(&native_draft).unwrap(), b"new native");
        assert!(draft.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discarding_unaccepted_pair_does_not_touch_existing_accepted_pair() {
        let root = fixture("discard");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("native_take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&native_draft, b"new native").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        fs::write(&native_target, b"old native").unwrap();
        discard_review_take(&native_draft).unwrap();
        discard_review_take(&draft).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"old canonical");
        assert_eq!(fs::read(&native_target).unwrap(), b"old native");
        fs::remove_dir_all(root).unwrap();
    }
}
