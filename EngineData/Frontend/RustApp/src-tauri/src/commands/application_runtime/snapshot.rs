use std::sync::atomic::{AtomicU64, Ordering};

use super::capabilities::resolve_capabilities;
use super::contract::{ApplicationInputStatus, ApplicationSnapshot};
use super::lifecycle::derive_lifecycle;
use super::problems::collect_problems;
use super::resources::resolve_resources;
use super::summaries::build_subsystem_summaries;
use super::super::{
    audio, helper_bridge, meeting_session, voice_lab, voice_lab_recording,
};

static APPLICATION_REVISION: AtomicU64 = AtomicU64::new(0);

pub fn current_application_snapshot() -> ApplicationSnapshot {
    // One observation per owner for this snapshot. Preflight may still use its
    // generation-bound Start receipt; an idle preflight reuses these inputs.
    let (settings, admission) = crate::engine::runtime_settings::load_settings_with_admission();
    let helper = helper_bridge::get_helper_bridge_status();
    let native_input = audio::get_input_status(&settings);
    let meeting = meeting_session::get_meeting_session_status_from_observations(
        &native_input, admission, &helper,
    );
    let input = ApplicationInputStatus {
        ready: native_input.prepared,
        prepared: native_input.prepared,
        functional_verified: native_input.functional_verified,
        callback_frames_observed: native_input.callback_frames_observed,
        selected_device_name: native_input.input_device_name.clone(),
        input_device_name: native_input.input_device_name.clone(),
        blocker: native_input.blocker.clone(),
        note: native_input.note.clone(),
    };
    let voice_build = voice_lab::current_voice_lab_build_snapshot();
    let voice_recording_active = voice_lab_recording::voice_lab_recording_active();
    // Read only the existing selected-actor predicate, not the full build
    // status (recordings, evaluation samples, and recovery suggestions).
    let approved_voice_ready = if voice_build.phase == "state_unavailable" {
        None
    } else {
        let paths = crate::engine::paths::ProjectPaths::discover();
        if paths.packaged_context_initialized || paths.development_root_verified {
            let storage = voice_lab::VoiceLabStoragePaths::from_project_paths(&paths);
            Some(voice_lab::approved_voice_actor_ready(&storage))
        } else {
            None
        }
    };
    let summaries = build_subsystem_summaries(
        &meeting,
        &helper,
        &native_input,
        &voice_build,
        voice_recording_active,
        approved_voice_ready,
    );
    let resources = resolve_resources(&summaries);
    let lifecycle = derive_lifecycle(&summaries);
    let capabilities = resolve_capabilities(&summaries, &resources);
    let problems = collect_problems(&summaries);

    ApplicationSnapshot {
        revision: APPLICATION_REVISION.fetch_add(1, Ordering::AcqRel) + 1,
        lifecycle,
        active_owner: summaries.meeting.owner_id.clone(),
        settings,
        meeting,
        helper,
        input,
        summaries,
        resources,
        capabilities,
        problems,
    }
}
