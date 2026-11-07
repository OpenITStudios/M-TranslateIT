use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::logging::sanitize_diagnostic_text;
use crate::engine::paths::ProjectPaths;

const INCIDENT_FILE: &str = "runtime_incidents.json";
const MAX_INCIDENTS: usize = 20;
const MAX_REVIEW_BYTES: u64 = 64 * 1024;
const INCIDENT_REPEAT_SUPPRESSION_MS: u128 = 60_000;
static INCIDENT_IO_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static INCIDENT_STAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static INCIDENT_RECORDING_SUPPRESSED: Cell<bool> = const { Cell::new(false) };
}

struct IncidentRecordingSuppressionGuard {
    previous: bool,
}

impl Drop for IncidentRecordingSuppressionGuard {
    fn drop(&mut self) {
        INCIDENT_RECORDING_SUPPRESSED.with(|flag| flag.set(self.previous));
    }
}

pub(super) fn without_runtime_incident_recording<T>(action: impl FnOnce() -> T) -> T {
    let previous = INCIDENT_RECORDING_SUPPRESSED.with(|flag| flag.replace(true));
    let _guard = IncidentRecordingSuppressionGuard { previous };
    action()
}

fn incident_recording_suppressed() -> bool {
    INCIDENT_RECORDING_SUPPRESSED.with(Cell::get)
}

fn incident_io_lock() -> &'static Mutex<()> {
    INCIDENT_IO_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeIncident {
    pub category: String,
    pub component: String,
    pub blocker: String,
    pub note: String,
    pub occurred_unix_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_source_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuntimeIssueArea {
    pub category: String,
    pub component: String,
    pub build_source_sha: Option<String>,
    // These are retained records, not total failures: repeated events are suppressed.
    pub retained_records: usize,
    pub blockers: Vec<String>,
    pub latest_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeIncidentSnapshot {
    pub incidents: Vec<RuntimeIncident>,
    pub count: usize,
    pub truncated: bool,
    pub coverage: &'static str,
    pub issue_areas: Vec<RuntimeIssueArea>,
    pub note: String,
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn incident_path() -> PathBuf {
    let paths = ProjectPaths::discover();
    PathBuf::from(paths.user_log_dir).join(INCIDENT_FILE)
}

fn read_incidents(path: &Path) -> io::Result<Vec<RuntimeIncident>> {
    Ok(inspect_incidents(path)?.unwrap_or_default())
}

// Publish one fully written, newly-created staging file. No open/truncate of
// an existing *.tmp file: interrupted prior attempts and unrelated files stay
// intact. The existing incident IO mutex serializes writers in this process.
fn publish_incident_stage(path: &Path, temp: &Path, body: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp)?;
    let written = file.write_all(body).and_then(|_| file.sync_all());
    drop(file);
    if let Err(error) = written {
        let _ = fs::remove_file(temp); // This writer's newly-created file only.
        return Err(error);
    }
    match fs::rename(temp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(temp); // The old published history is untouched.
            Err(error)
        }
    }
}

fn write_incidents(path: &Path, incidents: &[RuntimeIncident]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "incident path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let body = serde_json::to_vec_pretty(incidents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if body.len() as u64 > MAX_REVIEW_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "incident history too large"));
    }
    let nonce = INCIDENT_STAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = path.with_extension(format!(
        "json.tmp.{}.{}.{}",
        std::process::id(), unix_ms(), nonce
    ));
    publish_incident_stage(path, &temp, &body)
}

// Fail closed: corrupt/oversized previous evidence must never be overwritten.
fn persist_incident(path: &Path, next: RuntimeIncident) -> io::Result<()> {
    let mut incidents = read_incidents(path)?;
    if let Some(index) = incidents.iter().position(|current| {
        current.category == next.category
            && current.component == next.component
            && current.blocker == next.blocker
            && current.build_source_sha == next.build_source_sha
    }) {
        let previous = &incidents[index];
        if next.occurred_unix_ms.saturating_sub(previous.occurred_unix_ms)
            < INCIDENT_REPEAT_SUPPRESSION_MS
        {
            return Ok(());
        }
        incidents.remove(index);
    }
    incidents.insert(0, next);
    incidents.truncate(MAX_INCIDENTS);
    write_incidents(path, &incidents)
}

pub fn record_runtime_incident(
    category: &str,
    component: &str,
    blocker: &str,
    note: &str,
) -> bool {
    if incident_recording_suppressed() {
        return true;
    }
    let category = sanitize_diagnostic_text(category);
    let component = sanitize_diagnostic_text(component);
    let blocker = sanitize_diagnostic_text(blocker);
    let note = sanitize_diagnostic_text(note);
    if blocker.trim().is_empty() {
        return false;
    }
    let Ok(_guard) = incident_io_lock().lock() else { return false; };
    persist_incident(&incident_path(), RuntimeIncident {
        category, component, blocker, note,
        occurred_unix_ms: unix_ms(),
        build_source_sha: option_env!("TRANSLATEIT_BUILD_SOURCE_SHA").map(str::to_string),
    }).is_ok()
}

// Analytics is a bounded, read-only projection of the existing incident log.
// Missing history, malformed history and the 20-record retention ceiling must not
// be conflated with a verified healthy runtime or complete failure frequency.
fn inspect_incidents(path: &Path) -> io::Result<Option<Vec<RuntimeIncident>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > MAX_REVIEW_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid incident file"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(MAX_REVIEW_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_REVIEW_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid incident size"));
    }
    let incidents: Vec<RuntimeIncident> = serde_json::from_slice(&bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid incident JSON"))?;
    if incidents.len() > MAX_INCIDENTS {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unbounded incident history"));
    }
    let mut identities = HashSet::new();
    let valid_field = |text: &str| !text.trim().is_empty() && text.chars().count() <= 360;
    for item in &incidents {
        let source_valid = item.build_source_sha.as_deref().map_or(true, |sha| {
            sha.len() == 40 &&
                sha.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        });
        if !valid_field(&item.category) || !valid_field(&item.component)
            || !valid_field(&item.blocker) || item.note.chars().count() > 360
            || item.occurred_unix_ms == 0 || !source_valid
            || !identities.insert((&item.category, &item.component, &item.blocker, &item.build_source_sha))
        {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid incident metadata"));
        }
    }
    Ok(Some(incidents))
}

fn group_issue_areas(incidents: &[RuntimeIncident]) -> Vec<RuntimeIssueArea> {
    let mut grouped: BTreeMap<(String, String, Option<String>), RuntimeIssueArea> = BTreeMap::new();
    for item in incidents {
        let key = (item.category.clone(), item.component.clone(), item.build_source_sha.clone());
        let area = grouped.entry(key).or_insert_with(|| RuntimeIssueArea {
            category: item.category.clone(),
            component: item.component.clone(),
            build_source_sha: item.build_source_sha.clone(),
            retained_records: 0,
            blockers: Vec::new(),
            latest_unix_ms: 0,
        });
        area.retained_records += 1;
        area.latest_unix_ms = area.latest_unix_ms.max(item.occurred_unix_ms);
        if !area.blockers.contains(&item.blocker) {
            area.blockers.push(item.blocker.clone());
        }
    }
    let mut areas: Vec<_> = grouped.into_values().collect();
    areas.sort_by(|a, b| b.latest_unix_ms.cmp(&a.latest_unix_ms)
        .then(a.category.cmp(&b.category))
        .then(a.component.cmp(&b.component)));
    areas
}

#[tauri::command]
pub fn get_recent_runtime_incidents() -> RuntimeIncidentSnapshot {
    let (incidents, coverage) = match incident_io_lock().lock() {
        Ok(_guard) => match inspect_incidents(&incident_path()) {
            Ok(None) => (Vec::new(), "NO_DATA"),
            Ok(Some(records)) if records.len() >= MAX_INCIDENTS => (records, "AT_CAPACITY"),
            Ok(Some(records)) => (records, "RECENT_ONLY"),
            Err(_) => (Vec::new(), "UNAVAILABLE"),
        },
        Err(_) => (Vec::new(), "UNAVAILABLE"),
    };
    RuntimeIncidentSnapshot {
        count: incidents.len(),
        truncated: incidents.len() >= MAX_INCIDENTS,
        coverage,
        issue_areas: group_issue_areas(&incidents),
        incidents,
        note: "Read-only grouped recent incidents by component and optional build source. Older/unknown builds, suppressed repeats and expired history are not measured. No audio or transcript is collected.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        group_issue_areas, incident_recording_suppressed, inspect_incidents, persist_incident,
        publish_incident_stage, write_incidents,
        without_runtime_incident_recording, RuntimeIncident, INCIDENT_REPEAT_SUPPRESSION_MS,
        MAX_INCIDENTS, MAX_REVIEW_BYTES,
    };

    use std::{fs, time::{SystemTime, UNIX_EPOCH}};

    fn test_path() -> std::path::PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("translateit-incident-{}-{stamp}.json", std::process::id()))
    }

    fn same_incident_at(occurred_unix_ms: u128) -> RuntimeIncident {
        RuntimeIncident {
            category: "runtime_watchdog".to_string(),
            component: "meeting_outbound".to_string(),
            blocker: "runtime_watchdog:outbound_progress_stalled".to_string(),
            note: "stalled".to_string(),
            occurred_unix_ms,
            build_source_sha: None,
        }
    }


    #[test]
    fn incident_recording_suppression_is_scoped_and_restored() {
        assert!(!incident_recording_suppressed());
        let suppressed = without_runtime_incident_recording(incident_recording_suppressed);
        assert!(suppressed);
        assert!(!incident_recording_suppressed());
    }

    #[test]
    fn issue_map_groups_only_retained_records_without_inventing_failure_frequency() {
        let items = [
            same_incident_at(3),
            RuntimeIncident { category: "runtime_watchdog".to_string(),
                component: "meeting_outbound".to_string(),
                blocker: "runtime_watchdog:another_issue".to_string(),
                note: "internal detail".to_string(), occurred_unix_ms: 2,
                build_source_sha: None },
            RuntimeIncident { category: "device_loss".to_string(), component: "microphone".to_string(),
                blocker: "device_loss:microphone_capture_inactive".to_string(),
                note: "private path must not be projected".to_string(), occurred_unix_ms: 1,
                build_source_sha: None },
        ];
        let mapped = group_issue_areas(&items);
        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped[0].retained_records, 2);
        assert_eq!(mapped[0].latest_unix_ms, 3);
        assert_eq!(mapped[0].blockers.len(), 2);
        let encoded = serde_json::to_string(&mapped).unwrap();
        assert!(!encoded.contains("internal detail"));
        assert!(!encoded.contains("private path"));
    }

    #[test]
    fn issue_map_separates_recorded_builds_from_legacy_unknowns() {
        let mut old = same_incident_at(1);
        old.build_source_sha = Some("a".repeat(40));
        let mut new = same_incident_at(2);
        new.build_source_sha = Some("b".repeat(40));
        let mapped = group_issue_areas(&[old, new, same_incident_at(3)]);
        assert_eq!(mapped.len(), 3);
        assert_eq!(mapped[0].build_source_sha, None);
        assert_eq!(mapped[1].build_source_sha.as_deref(), Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        assert_eq!(mapped[2].build_source_sha.as_deref(), Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    }

    #[test]
    fn invalid_incident_history_cannot_be_erased_by_new_events() {
        let path = test_path();
        fs::write(&path, b"{invalid-json").unwrap();
        assert!(persist_incident(&path, same_incident_at(4)).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{invalid-json");
        fs::write(&path, vec![b' '; MAX_REVIEW_BYTES as usize + 1]).unwrap();
        assert!(inspect_incidents(&path).is_err());
        assert!(persist_incident(&path, same_incident_at(5)).is_err());
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn legacy_incidents_survive_bounded_append_without_build_forgery() {
        let path = test_path();
        let old = same_incident_at(2);
        fs::write(&path, serde_json::to_vec(&vec![old.clone()]).unwrap()).unwrap();
        let mut new = same_incident_at(3);
        new.build_source_sha = Some("c".repeat(40));
        persist_incident(&path, new).unwrap();
        let rows = inspect_incidents(&path).unwrap().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1], old);
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn existing_history_is_replaced_without_deleting_before_publication() {
        let path = test_path();
        let original = same_incident_at(1);
        write_incidents(&path, &[original]).unwrap();
        let updated = same_incident_at(2);
        write_incidents(&path, &[updated.clone()]).unwrap();
        assert_eq!(inspect_incidents(&path).unwrap().unwrap(), vec![updated]);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn failed_staging_preserves_previous_incident_history() {
        let path = test_path();
        let original = same_incident_at(1);
        write_incidents(&path, &[original.clone()]).unwrap();
        let before = fs::read(&path).unwrap();
        let temp = path.with_extension("json.tmp.blocked");
        fs::create_dir(&temp).unwrap();
        let next = serde_json::to_vec_pretty(&vec![same_incident_at(2)]).unwrap();
        assert!(publish_incident_stage(&path, &temp, &next).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(inspect_incidents(&path).unwrap().unwrap(), vec![original]);
        fs::remove_dir(temp).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn preexisting_stage_file_is_not_truncated_or_followed() {
        let path = test_path();
        write_incidents(&path, &[same_incident_at(1)]).unwrap();
        let prior = fs::read(&path).unwrap();
        let temp = path.with_extension("json.tmp.occupied");
        fs::write(&temp, b"unrelated preexisting evidence").unwrap();
        let next = serde_json::to_vec_pretty(&vec![same_incident_at(2)]).unwrap();
        let error = publish_incident_stage(&path, &temp, &next).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&temp).unwrap(), b"unrelated preexisting evidence");
        assert_eq!(fs::read(&path).unwrap(), prior);
        fs::remove_file(temp).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn incident_bound_remains_small() {
        assert_eq!(MAX_INCIDENTS, 20);
        assert!(std::mem::size_of::<RuntimeIncident>() < 256);
    }

    #[test]
    fn repeat_suppression_window_covers_live_poll_cadence() {
        assert!(INCIDENT_REPEAT_SUPPRESSION_MS >= 30_000);
        assert!(INCIDENT_REPEAT_SUPPRESSION_MS <= 300_000);
    }

    #[test]
    fn same_incident_identity_is_searchable_beyond_front_entry() {
        let incidents = [
            RuntimeIncident {
                category: "device_loss".to_string(),
                component: "microphone".to_string(),
                blocker: "device_loss:microphone_capture_inactive".to_string(),
                note: "lost".to_string(),
                occurred_unix_ms: 2,
                build_source_sha: None,
            },
            same_incident_at(1),
        ];
        let next = same_incident_at(10);
        let index = incidents.iter().position(|current| {
            current.category == next.category
                && current.component == next.component
                && current.blocker == next.blocker
        });
        assert_eq!(index, Some(1));
    }
}
