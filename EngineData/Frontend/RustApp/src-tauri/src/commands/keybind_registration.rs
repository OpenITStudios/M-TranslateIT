//! Pure registration transaction planner. No OS hooks, shortcut dispatch,
//! Tauri commands, persistence, or permissions. A future native adapter must
//! supply actual acquire/release/rollback evidence before enabling shortcuts.
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistrationPlan {
    pub acquire: Vec<String>,
    pub release: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistrationError {
    DuplicateChord,
    InvalidChord,
    AcquireFailed,
    RollbackFailed,
    ReleaseFailed,
    PersistenceFailed,
}

/// Abstract registrar for deterministic unit tests. Production adapters must
/// implement OS-specific registration and keep a verified ownership ledger.
pub trait ShortcutRegistrar {
    fn acquire(&mut self, chord: &str) -> Result<(), ()>;
    fn release(&mut self, chord: &str) -> Result<(), ()>;
}

fn checked_set(chords: &[String]) -> Result<BTreeSet<String>, RegistrationError> {
    let mut set = BTreeSet::new();
    for chord in chords {
        if chord.is_empty() || chord.trim() != chord || !set.insert(chord.clone()) {
            return Err(if chord.is_empty() || chord.trim() != chord {
                RegistrationError::InvalidChord
            } else {
                RegistrationError::DuplicateChord
            });
        }
    }
    Ok(set)
}

pub fn plan_registration(
    current: &[String],
    candidate: &[String],
) -> Result<RegistrationPlan, RegistrationError> {
    let old = checked_set(current)?;
    let new = checked_set(candidate)?;
    Ok(RegistrationPlan {
        acquire: new.difference(&old).cloned().collect(),
        release: old.difference(&new).cloned().collect(),
    })
}

/// Only the acquisition phase is executable in this isolated owner.
/// A future integrated transaction must coordinate persistence, release,
/// action dispatch, and rollback of both OS and disk state.
pub fn stage_acquisition<R: ShortcutRegistrar>(
    registrar: &mut R,
    plan: &RegistrationPlan,
) -> Result<(), RegistrationError> {
    let mut acquired: Vec<&str> = Vec::new();
    for chord in &plan.acquire {
        if registrar.acquire(chord).is_err() {
            let mut rollback_failed = false;
            for registered in acquired.into_iter().rev() {
                if registrar.release(registered).is_err() {
                    rollback_failed = true;
                }
            }
            return Err(if rollback_failed {
                RegistrationError::RollbackFailed
            } else {
                RegistrationError::AcquireFailed
            });
        }
        acquired.push(chord);
    }
    Ok(())
}

/// Persistence boundary for a future native adapter. The caller must hold
/// its own process-wide transaction lock and disable dispatch while applying.
pub trait ShortcutPreferenceStore {
    fn persist_candidate(&mut self) -> Result<(), ()>;
    fn restore_previous(&mut self) -> Result<(), ()>;
}

/// An isolated transaction model, not a live shortcut manager. The adapter
/// must ensure registration and persistence share the same ownership lock.
/// On any uncertain rollback, the caller must enter a disabled/degraded state.
pub fn apply_registration_transaction<R: ShortcutRegistrar, S: ShortcutPreferenceStore>(
    registrar: &mut R,
    store: &mut S,
    plan: &RegistrationPlan,
) -> Result<(), RegistrationError> {
    let mut acquired: Vec<&str> = Vec::new();
    for chord in &plan.acquire {
        if registrar.acquire(chord).is_err() {
            let rollback_failed = acquired.iter().rev()
                .any(|registered| registrar.release(registered).is_err());
            return Err(if rollback_failed { RegistrationError::RollbackFailed } else { RegistrationError::AcquireFailed });
        }
        acquired.push(chord);
    }

    if store.persist_candidate().is_err() {
        let mut rollback_failed = false;
        for chord in acquired.iter().rev() {
            if registrar.release(chord).is_err() { rollback_failed = true; }
        }
        if store.restore_previous().is_err() { rollback_failed = true; }
        return Err(if rollback_failed { RegistrationError::RollbackFailed } else { RegistrationError::PersistenceFailed });
    }

    // The previous preference is already durable. A failed release must not
    // pretend success: attempt to restore old disk state and release new chords.
    let mut released_old: Vec<&str> = Vec::new();
    for chord in &plan.release {
        if registrar.release(chord).is_err() {
            let mut rollback_failed = false;
            for old in released_old.iter().rev() {
                if registrar.acquire(old).is_err() { rollback_failed = true; }
            }
            for new in acquired.iter().rev() {
                if registrar.release(new).is_err() { rollback_failed = true; }
            }
            if store.restore_previous().is_err() { rollback_failed = true; }
            return Err(if rollback_failed { RegistrationError::RollbackFailed } else { RegistrationError::ReleaseFailed });
        }
        released_old.push(chord);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Fake {
        active: BTreeSet<String>,
        fail_acquire: Option<String>,
        fail_release: Option<String>,
    }
    impl ShortcutRegistrar for Fake {
        fn acquire(&mut self, chord: &str) -> Result<(), ()> {
            if self.fail_acquire.as_deref() == Some(chord) { return Err(()); }
            self.active.insert(chord.to_string());
            Ok(())
        }
        fn release(&mut self, chord: &str) -> Result<(), ()> {
            if self.fail_release.as_deref() == Some(chord) { return Err(()); }
            self.active.remove(chord);
            Ok(())
        }
    }
    #[test]
    fn unchanged_chords_do_not_reacquire() {
        let plan = plan_registration(&["Ctrl+Shift+M".into()], &["Ctrl+Shift+M".into()]).unwrap();
        assert!(plan.acquire.is_empty());
        assert!(plan.release.is_empty());
    }
    #[test]
    fn rejects_duplicate_candidate() {
        assert_eq!(
            plan_registration(&[], &["Ctrl+Shift+M".into(), "Ctrl+Shift+M".into()]),
            Err(RegistrationError::DuplicateChord),
        );
    }
    #[test]
    fn failed_acquisition_rolls_back_newly_acquired_only() {
        let plan = plan_registration(&["Ctrl+Alt+S".into()],
            &["Ctrl+Alt+S".into(), "Ctrl+Shift+A".into(), "Ctrl+Shift+M".into()]).unwrap();
        let mut fake = Fake::default();
        fake.active.insert("Ctrl+Alt+S".into());
        fake.fail_acquire = Some("Ctrl+Shift+M".into());
        assert_eq!(stage_acquisition(&mut fake, &plan), Err(RegistrationError::AcquireFailed));
        assert_eq!(fake.active, BTreeSet::from(["Ctrl+Alt+S".into()]));
    }
    #[test]
    fn rollback_failure_is_not_reported_as_clean_failure() {
        let plan = plan_registration(&[], &["Ctrl+Shift+A".into(), "Ctrl+Shift+M".into()]).unwrap();
        let mut fake = Fake {
            fail_acquire: Some("Ctrl+Shift+M".into()),
            fail_release: Some("Ctrl+Shift+A".into()),
            ..Fake::default()
        };
        assert_eq!(stage_acquisition(&mut fake, &plan), Err(RegistrationError::RollbackFailed));
        assert!(fake.active.contains("Ctrl+Shift+A"));
    }
}
