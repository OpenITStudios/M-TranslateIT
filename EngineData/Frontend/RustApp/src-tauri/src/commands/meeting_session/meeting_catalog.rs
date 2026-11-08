// Meeting library is display/organization metadata only. Transcript bytes and
// Meeting lifecycle remain owned by transcript_archive and lifecycle.
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

use crate::engine::paths::ProjectPaths;
use super::transcript_archive::{self, SavedMeetingSession};

const SCHEMA: u32 = 1;
const MAX_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PROJECTS: usize = 128;
const MAX_SESSIONS: usize = 12_000;
static MUTATION: OnceLock<Mutex<()>> = OnceLock::new();
static NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize, Deserialize)]
pub struct MeetingProject {
    pub project_id: String,
    pub name: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct MeetingLabel {
    session_id: String,
    display_name: String,
    project_id: Option<String>,
    starred: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct Catalog {
    schema_version: u32,
    revision: u64,
    projects: Vec<MeetingProject>,
    sessions: Vec<MeetingLabel>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self { schema_version: SCHEMA, revision: 0, projects: vec![], sessions: vec![] }
    }
}

#[derive(Serialize)]
pub struct MeetingLibrarySession {
    pub session_id: String,
    pub display_name: String,
    pub project_id: Option<String>,
    pub starred: bool,
    pub entry_count: usize,
    pub complete: bool,
    pub storage_warning: bool,
}

#[derive(Serialize)]
pub struct MeetingLibrary {
    pub ok: bool,
    pub revision: u64,
    pub active_write_errors: u64,
    pub projects: Vec<MeetingProject>,
    pub sessions: Vec<MeetingLibrarySession>,
    pub message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingLibraryChange {
    pub action: String,
    pub expected_revision: u64,
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub name: Option<String>,
    pub starred: Option<bool>,
}

fn lock() -> &'static Mutex<()> { MUTATION.get_or_init(|| Mutex::new(())) }
fn catalog_path() -> PathBuf {
    PathBuf::from(ProjectPaths::discover().user_saved_dir)
        .join("Transcripts").join("Library").join("catalog.json")
}
fn valid_session_id(id: &str) -> bool {
    id.starts_with("meeting_") && id.len() <= 96
        && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn valid_project_id(id: &str) -> bool {
    id.starts_with("project_") && id.len() <= 96
        && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn name(value: Option<&str>) -> Result<String, String> {
    let name = value.unwrap_or("").trim();
    if name.is_empty() || name.chars().count() > 80
        || name.chars().any(char::is_control) {
        return Err("Enter a name between 1 and 80 characters without control characters.".into());
    }
    Ok(name.to_string())
}
fn validate_catalog(c: &Catalog) -> Result<(), String> {
    if c.schema_version != SCHEMA || c.projects.len() > MAX_PROJECTS || c.sessions.len() > MAX_SESSIONS {
        return Err("Meeting library metadata version or size is unsupported.".into());
    }
    let mut project_ids = std::collections::HashSet::new();
    for p in &c.projects {
        if !valid_project_id(&p.project_id) || name(Some(&p.name)).is_err()
            || !project_ids.insert(p.project_id.as_str()) {
            return Err("Meeting project metadata is invalid.".into());
        }
    }
    let mut session_ids = std::collections::HashSet::new();
    for m in &c.sessions {
        if !valid_session_id(&m.session_id) || name(Some(&m.display_name)).is_err()
            || !session_ids.insert(m.session_id.as_str()) {
            return Err("Meeting label metadata is invalid.".into());
        }
        if m.project_id.as_deref().is_some_and(|id| !project_ids.contains(id)) {
            return Err("Meeting references an unknown project.".into());
        }
    }
    Ok(())
}
fn reject_symlink(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(info) if info.file_type().is_symlink() => Err("Meeting library location cannot be a symbolic link.".into()),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Meeting library path could not be inspected.".into()),
    }
}
fn read_bounded(path: &Path) -> Result<Catalog, String> {
    reject_symlink(path)?;
    let mut file = File::open(path).map_err(|_| "Meeting library metadata is unreadable.")?;
    if file.metadata().map_err(|_| "Meeting library metadata cannot be inspected.")?.len() > MAX_BYTES {
        return Err("Meeting library metadata exceeds its safety limit.".into());
    }
    let mut bytes = Vec::new();
    (&mut file).take(MAX_BYTES + 1).read_to_end(&mut bytes)
        .map_err(|_| "Meeting library metadata could not be read.")?;
    if bytes.len() as u64 > MAX_BYTES { return Err("Meeting library metadata is oversized.".into()); }
    let c: Catalog = serde_json::from_slice(&bytes)
        .map_err(|_| "Meeting library metadata is damaged. Its archives were not changed.")?;
    validate_catalog(&c)?;
    Ok(c)
}
fn load(path: &Path) -> Result<Catalog, String> {
    let backup = path.with_extension("json.bak");
    match read_bounded(path) {
        Ok(c) => Ok(c),
        Err(_) if !path.exists() && backup.exists() => read_bounded(&backup),
        Err(_) if !path.exists() && !backup.exists() && !path.with_extension("json.tmp").exists() => Ok(Catalog::default()),
        Err(e) => Err(e),
    }
}
fn write(path: &Path, c: &Catalog) -> Result<(), String> {
    validate_catalog(c)?;
    let content = serde_json::to_vec_pretty(c)
        .map_err(|_| "Meeting library metadata could not be serialized.")?;
    if content.len() as u64 > MAX_BYTES { return Err("Meeting library metadata is too large.".into()); }
    let parent = path.parent().ok_or("Invalid Meeting library location.")?;
    fs::create_dir_all(parent).map_err(|_| "Meeting library directory is not writable.")?;
    let tmp = path.with_extension("json.tmp");
    let bak = path.with_extension("json.bak");
    for location in [parent, path, tmp.as_path(), bak.as_path()] {
        reject_symlink(location)?;
    }
    // Never replace the last recoverable copy until the new file exists.
    {
        let mut file = File::create(&tmp).map_err(|_| "Could not stage Meeting library metadata.")?;
        file.write_all(&content).and_then(|_| file.sync_all())
            .map_err(|_| "Could not persist staged Meeting library metadata.")?;
    }
    if path.exists() {
        fs::copy(path, &bak).map_err(|_| "Could not back up Meeting library metadata.")?;
    }
    match fs::rename(&tmp, path) {
        Ok(()) => (),
        Err(_) => {
            // Windows does not replace an existing file with rename.
            if path.exists() {
                fs::remove_file(path).map_err(|_| "Meeting library cannot replace old metadata.")?;
            }
            if fs::rename(&tmp, path).is_err() {
                if bak.exists() { let _ = fs::copy(&bak, path); }
                return Err("Meeting library update failed; its previous metadata was retained for recovery.".into());
            }
        }
    }
    let verified = read_bounded(path)?;
    if verified.revision != c.revision { return Err("Meeting library save could not be verified.".into()); }
    let _ = fs::remove_file(bak);
    Ok(())
}
fn default_title(session_id: &str) -> String {
    // Display-only UTC date derived from native session identity; source files
    // are never renamed. Custom names are independent of this fallback.
    let Some(ms) = session_id.split('_').nth(1).and_then(|v| v.parse::<u64>().ok()) else {
        return "Untitled Meeting".to_string();
    };
    let seconds = ms / 1000;
    let days = (seconds / 86_400) as i64;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    if month <= 2 { year += 1; }
    let hour = (seconds % 86_400) / 3600;
    let minute = (seconds % 3600) / 60;
    format!("Meeting {year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}
fn view_saved(saved: SavedMeetingSession, c: &Catalog) -> MeetingLibrarySession {
    let metadata = c.sessions.iter().find(|m| m.session_id == saved.session_id);
    MeetingLibrarySession {
        display_name: metadata.map(|m| m.display_name.clone())
            .unwrap_or_else(|| default_title(&saved.session_id)),
        project_id: metadata.and_then(|m| m.project_id.clone()),
        starred: metadata.is_some_and(|m| m.starred),
        session_id: saved.session_id,
        entry_count: saved.entry_count, complete: saved.complete,
        storage_warning: saved.storage_warning,
    }
}
fn failure(message: String) -> MeetingLibrary {
    MeetingLibrary { ok: false, revision: 0, active_write_errors: 0, projects: vec![], sessions: vec![], message }
}
pub fn list() -> MeetingLibrary {
    let saved = transcript_archive::list_saved_sessions();
    if !saved.ok { return failure(if saved.message.is_empty() {
        "Saved Meeting archives could not be verified.".into() } else { saved.message }); }
    let c = match load(&catalog_path()) {
        Ok(c) => c,
        Err(message) => {
            // Damaged category metadata must not hide or mutate intact archives.
            // Present the disk inventory with names derived from immutable IDs;
            // block metadata mutations until the catalog can be repaired.
            let defaults = Catalog::default();
            return MeetingLibrary {
                ok: false, revision: 0, active_write_errors: saved.active_write_errors,
                projects: vec![],
                sessions: saved.sessions.into_iter().map(|s| view_saved(s, &defaults)).collect(),
                message,
            };
        }
    };
    MeetingLibrary {
        ok: true, revision: c.revision, active_write_errors: saved.active_write_errors,
        projects: c.projects.clone(),
        sessions: saved.sessions.into_iter().map(|s| view_saved(s, &c)).collect(),
        message: String::new(),
    }
}
fn new_project_id(c: &Catalog) -> Result<String, String> {
    let time = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|_| "System clock cannot create a Project identity.")?.as_nanos();
    for _ in 0..4 {
        let id = format!("project_{time:x}_{:x}", NONCE.fetch_add(1, Ordering::Relaxed));
        if !c.projects.iter().any(|p| p.project_id == id) { return Ok(id); }
    }
    Err("Could not allocate a unique Project identity.".into())
}
fn apply(c: &mut Catalog, change: MeetingLibraryChange, archived: &[SavedMeetingSession]) -> Result<(), String> {
    let sid = change.session_id.as_deref();
    let pid = change.project_id.as_deref();
    match change.action.as_str() {
        "create_project" => {
            if c.projects.len() >= MAX_PROJECTS { return Err("Too many Project categories.".into()); }
            let title = name(change.name.as_deref())?;
            let id = new_project_id(c)?;
            c.projects.push(MeetingProject { project_id: id, name: title });
        }
        "rename_project" => {
            let title = name(change.name.as_deref())?;
            let project = c.projects.iter_mut().find(|p| Some(p.project_id.as_str()) == pid)
                .ok_or("Project no longer exists.")?;
            project.name = title;
        }
        "delete_project" => {
            let id = pid.ok_or("Choose a Project.")?;
            if !c.projects.iter().any(|p| p.project_id == id) { return Err("Project no longer exists.".into()); }
            c.projects.retain(|p| p.project_id != id);
            for item in &mut c.sessions { if item.project_id.as_deref() == Some(id) { item.project_id = None; } }
        }
        "rename_meeting" | "star_meeting" | "move_meeting" => {
            let id = sid.filter(|s| valid_session_id(s)).ok_or("Invalid Meeting identity.")?;
            if !archived.iter().any(|s| s.session_id == id) {
                return Err("Meeting archive is unavailable. Refresh the library.".into());
            }
            let position = c.sessions.iter().position(|s| s.session_id == id);
            if position.is_none() {
                if c.sessions.len() >= MAX_SESSIONS { return Err("Meeting library has reached its metadata limit.".into()); }
                c.sessions.push(MeetingLabel {
                    session_id: id.to_string(), display_name: default_title(id),
                    project_id: None, starred: false,
                });
            }
            let item = c.sessions.iter_mut().find(|m| m.session_id == id).expect("session exists");
            match change.action.as_str() {
                "rename_meeting" => item.display_name = name(change.name.as_deref())?,
                "star_meeting" => item.starred = change.starred.ok_or("Choose a Starred state.")?,
                _ => {
                    if let Some(id) = pid {
                        if !c.projects.iter().any(|p| p.project_id == id) {
                            return Err("Project no longer exists.".into());
                        }
                    }
                    item.project_id = change.project_id;
                }
            }
        }
        _ => return Err("Unsupported Meeting library action.".into()),
    }
    Ok(())
}
pub fn mutate(change: MeetingLibraryChange) -> MeetingLibrary {
    // Same order as archive delete: saved-session guard -> metadata guard.
    let _archive_guard = match transcript_archive::saved_session_operation_guard() {
        Ok(g) => g, Err(message) => return failure(message),
    };
    let _metadata_guard = match lock().lock() {
        Ok(g) => g, Err(_) => return failure("Meeting library metadata lock unavailable.".into()),
    };
    let saved = transcript_archive::list_saved_sessions();
    if !saved.ok { return failure(saved.message); }
    let path = catalog_path();
    let mut c = match load(&path) { Ok(c) => c, Err(e) => return failure(e) };
    if c.revision != change.expected_revision { return failure("Meeting library changed; refresh before trying again.".into()); }
    if let Err(e) = apply(&mut c, change, &saved.sessions) { return failure(e); }
    let Some(next) = c.revision.checked_add(1) else { return failure("Meeting library revision exhausted.".into()) };
    c.revision = next;
    if let Err(e) = write(&path, &c) { return failure(e); }
    MeetingLibrary {
        ok: true, revision: c.revision, active_write_errors: saved.active_write_errors,
        projects: c.projects.clone(),
        sessions: saved.sessions.into_iter().map(|s| view_saved(s, &c)).collect(),
        message: "Meeting library updated.".into(),
    }
}
pub(super) fn forget_deleted_session(session_id: &str) -> Result<(), String> {
    let _guard = lock().lock().map_err(|_| "Meeting metadata lock unavailable.")?;
    let path = catalog_path();
    let mut c = load(&path)?;
    let old_len = c.sessions.len();
    c.sessions.retain(|item| item.session_id != session_id);
    if old_len == c.sessions.len() { return Ok(()); }
    c.revision = c.revision.checked_add(1).ok_or("Meeting metadata revision exhausted.")?;
    write(&path, &c)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_and_identifiers_are_bounded() {
        assert!(name(Some("  Weekly sync  ")).is_ok());
        assert!(name(Some("\n")).is_err());
        assert!(name(Some(&"a".repeat(81))).is_err());
        assert!(!valid_session_id("meeting_../private"));
        assert!(!valid_project_id("project_../private"));
    }
    #[test]
    fn rename_move_and_star_preserve_immutable_archived_session_identity() {
        let id = "meeting_1760000000000_9";
        let archived = [SavedMeetingSession {
            session_id: id.into(), entry_count: 12, complete: true, storage_warning: false,
        }];
        let mut c = Catalog::default();
        apply(&mut c, MeetingLibraryChange {
            action: "rename_meeting".into(), expected_revision: 0,
            session_id: Some(id.into()), project_id: None,
            name: Some("Planning discussion".into()), starred: None,
        }, &archived).unwrap();
        assert_eq!(c.sessions[0].session_id, id);
        assert_eq!(c.sessions[0].display_name, "Planning discussion");
        apply(&mut c, MeetingLibraryChange {
            action: "star_meeting".into(), expected_revision: 0,
            session_id: Some(id.into()), project_id: None,
            name: None, starred: Some(true),
        }, &archived).unwrap();
        assert!(c.sessions[0].starred);
        assert_eq!(c.sessions[0].session_id, id);
    }

    #[test]
    fn project_delete_does_not_delete_meeting_identity() {
        let mut c = Catalog::default();
        c.projects.push(MeetingProject { project_id:"project_test".into(), name:"Test".into() });
        c.sessions.push(MeetingLabel {session_id:"meeting_123".into(),display_name:"My call".into(),
            project_id:Some("project_test".into()),starred:true});
        apply(&mut c, MeetingLibraryChange {action:"delete_project".into(), expected_revision:0,
            session_id:None,project_id:Some("project_test".into()),name:None,starred:None}, &[]).unwrap();
        assert!(c.projects.is_empty());
        assert_eq!(c.sessions[0].project_id, None);
        assert_eq!(c.sessions[0].display_name, "My call");
        assert!(c.sessions[0].starred);
    }
}
