use std::path::PathBuf;

use crate::engine::paths::ProjectPaths;
pub use crate::engine::settings::RuntimeSettings;
use crate::engine::settings::MeetingEngineAdmission;

fn settings_path() -> PathBuf {
    let project_paths = ProjectPaths::discover();
    PathBuf::from(project_paths.user_cache_dir).join("rust_runtime_settings.json")
}

pub fn load_settings() -> RuntimeSettings {
    RuntimeSettings::load_or_default(&settings_path())
}

pub fn meeting_engine_admission() -> MeetingEngineAdmission {
    RuntimeSettings::meeting_engine_admission(&settings_path())
}
