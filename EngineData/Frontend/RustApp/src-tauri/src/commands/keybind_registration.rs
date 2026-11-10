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
            let mut rollback_failed = false;
            for registered in acquired.iter().rev() {
                if registrar.release(registered).is_err() { rollback_failed = true; }
            }
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

/// A fail-closed ownership state for a future OS adapter. This type does
/// not register real shortcuts or authorize dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistrationState {
    Unassigned,
    Active(BTreeSet<String>),
    Degraded,
}

pub struct RegistrationLifecycle {
    state: RegistrationState,
}

impl RegistrationLifecycle {
    pub fn new() -> Self {
        Self { state: RegistrationState::Unassigned }
    }

    pub fn state(&self) -> &RegistrationState { &self.state }

    /// Caller must serialize all operations and suspend event dispatch while
    /// this method runs. No operation is allowed after uncertain rollback.
    pub fn apply<R: ShortcutRegistrar, S: ShortcutPreferenceStore>(
        &mut self,
        registrar: &mut R,
        store: &mut S,
        candidate: &[String],
    ) -> Result<(), RegistrationError> {
        let current: Vec<String> = match &self.state {
            RegistrationState::Unassigned => Vec::new(),
            RegistrationState::Active(chords) => chords.iter().cloned().collect(),
            RegistrationState::Degraded => return Err(RegistrationError::RollbackFailed),
        };
        let plan = plan_registration(&current, candidate)?;
        match apply_registration_transaction(registrar, store, &plan) {
            Ok(()) => {
                let active = checked_set(candidate)?;
                self.state = if active.is_empty() {
                    RegistrationState::Unassigned
                } else {
                    RegistrationState::Active(active)
                };
                Ok(())
            }
            Err(error) => {
                if error == RegistrationError::RollbackFailed {
                    self.state = RegistrationState::Degraded;
                }
                Err(error)
            }
        }
    }

    /// Shutdown cleanup only reports success when every release succeeded.
    /// On failure, state becomes Degraded and no later action is admitted.
    pub fn shutdown<R: ShortcutRegistrar>(&mut self, registrar: &mut R) -> Result<(), RegistrationError> {
        let current = match &self.state {
            RegistrationState::Unassigned => return Ok(()),
            RegistrationState::Active(chords) => chords.clone(),
            RegistrationState::Degraded => return Err(RegistrationError::RollbackFailed),
        };
        let mut failed = false;
        for chord in &current {
            if registrar.release(chord).is_err() { failed = true; }
        }
        if failed {
            self.state = RegistrationState::Degraded;
            Err(RegistrationError::ReleaseFailed)
        } else {
            self.state = RegistrationState::Unassigned;
            Ok(())
        }
    }
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
    #[derive(Default)]
    struct FakeStore {
        fail_persist: bool,
        fail_restore: bool,
        persisted: bool,
    }
    impl ShortcutPreferenceStore for FakeStore {
        fn persist_candidate(&mut self) -> Result<(), ()> {
            if self.fail_persist { return Err(()); }
            self.persisted = true;
            Ok(())
        }
        fn restore_previous(&mut self) -> Result<(), ()> {
            if self.fail_restore { return Err(()); }
            self.persisted = false;
            Ok(())
        }
    }
    #[test]
    fn lifecycle_blocks_new_transactions_after_rollback_failure() {
        let mut lifecycle = RegistrationLifecycle::new();
        let mut registrar = Fake::default();
        let mut store = FakeStore { fail_persist: true, fail_restore: true, ..FakeStore::default() };
        assert_eq!(lifecycle.apply(&mut registrar, &mut store, &["Ctrl+Shift+M".into()]), Err(RegistrationError::RollbackFailed));
        assert_eq!(lifecycle.state(), &RegistrationState::Degraded);
        let mut fresh = FakeStore::default();
        assert_eq!(lifecycle.apply(&mut registrar, &mut fresh, &[]), Err(RegistrationError::RollbackFailed));
    }
    #[test]
    fn lifecycle_shutdown_releases_owned_chords() {
        let mut lifecycle = RegistrationLifecycle::new();
        let mut registrar = Fake::default();
        let mut store = FakeStore::default();
        lifecycle.apply(&mut registrar, &mut store, &["Ctrl+Shift+M".into()]).unwrap();
        assert_eq!(lifecycle.shutdown(&mut registrar), Ok(()));
        assert!(registrar.active.is_empty());
        assert_eq!(lifecycle.state(), &RegistrationState::Unassigned);
    }
    #[test]
    fn lifecycle_shutdown_failure_is_degraded() {
        let mut lifecycle = RegistrationLifecycle::new();
        let mut registrar = Fake::default();
        let mut store = FakeStore::default();
        lifecycle.apply(&mut registrar, &mut store, &["Ctrl+Shift+M".into()]).unwrap();
        registrar.fail_release = Some("Ctrl+Shift+M".into());
        assert_eq!(lifecycle.shutdown(&mut registrar), Err(RegistrationError::ReleaseFailed));
        assert_eq!(lifecycle.state(), &RegistrationState::Degraded);
    }
    #[test]
    fn transaction_commits_and_releases_old_chord() {
        let plan = plan_registration(&["Ctrl+Alt+S".into()], &["Ctrl+Shift+M".into()]).unwrap();
        let mut fake = Fake::default();
        fake.active.insert("Ctrl+Alt+S".into());
        let mut store = FakeStore::default();
        assert_eq!(apply_registration_transaction(&mut fake, &mut store, &plan), Ok(()));
        assert_eq!(fake.active, BTreeSet::from(["Ctrl+Shift+M".into()]));
        assert!(store.persisted);
    }
    #[test]
    fn persistence_failure_rolls_back_new_chord() {
        let plan = plan_registration(&["Ctrl+Alt+S".into()], &["Ctrl+Shift+M".into()]).unwrap();
        let mut fake = Fake::default();
        fake.active.insert("Ctrl+Alt+S".into());
        let mut store = FakeStore { fail_persist: true, ..FakeStore::default() };
        assert_eq!(apply_registration_transaction(&mut fake, &mut store, &plan), Err(RegistrationError::PersistenceFailed));
        assert_eq!(fake.active, BTreeSet::from(["Ctrl+Alt+S".into()]));
        assert!(!store.persisted);
    }
    #[test]
    fn release_failure_restores_previous_registration_and_disk_state() {
        let plan = plan_registration(&["Ctrl+Alt+S".into()], &["Ctrl+Shift+M".into()]).unwrap();
        let mut fake = Fake::default();
        fake.active.insert("Ctrl+Alt+S".into());
        fake.fail_release = Some("Ctrl+Alt+S".into());
        let mut store = FakeStore::default();
        assert_eq!(apply_registration_transaction(&mut fake, &mut store, &plan), Err(RegistrationError::ReleaseFailed));
        assert_eq!(fake.active, BTreeSet::from(["Ctrl+Alt+S".into()]));
        assert!(!store.persisted);
    }
    #[test]
    fn partial_old_release_failure_reacquires_already_released_chords() {
        let plan = plan_registration(
            &["Ctrl+Alt+A".into(), "Ctrl+Alt+B".into()],
            &["Ctrl+Shift+M".into()],
        ).unwrap();
        let mut fake = Fake::default();
        fake.active.extend(["Ctrl+Alt+A".into(), "Ctrl+Alt+B".into()]);
        fake.fail_release = Some("Ctrl+Alt+B".into());
        let mut store = FakeStore::default();
        assert_eq!(apply_registration_transaction(&mut fake, &mut store, &plan), Err(RegistrationError::ReleaseFailed));
        assert_eq!(fake.active, BTreeSet::from(["Ctrl+Alt+A".into(), "Ctrl+Alt+B".into()]));
        assert!(!store.persisted);
    }
    #[test]
    fn failed_disk_rollback_is_reported_as_degraded() {
        let plan = plan_registration(&[], &["Ctrl+Shift+M".into()]).unwrap();
        let mut fake = Fake::default();
        let mut store = FakeStore { fail_persist: true, fail_restore: true, persisted: false };
        assert_eq!(apply_registration_transaction(&mut fake, &mut store, &plan), Err(RegistrationError::RollbackFailed));
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
