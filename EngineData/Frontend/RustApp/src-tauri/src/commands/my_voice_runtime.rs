//! Shared exclusive native My Voice inference scope. The existing build
//! generation, helper permit and RuntimeOwner own every request and cleanup.
use super::helper_bridge_runtime::{
    acquire_helper_task_permit, clear_active_request, runtime, set_blocked,
    HelperTaskPriority, NativeInferenceOwner,
};
use super::voice_lab::current_voice_lab_build_snapshot;
use std::time::Instant;
use translateit_native_cuda::{InferenceInput, InferenceOutput};

pub(super) fn authority(generation: u64, deadline: Instant) -> bool {
    let current = current_voice_lab_build_snapshot();
    current.generation == Some(generation) && current.active && matches!(current.phase.as_str(), "preparing" | "evaluating")
        && !current.cancel_requested && Instant::now() < deadline
        && {
            let state = crate::engine::runtime_state::latest_runtime_session_state();
            !state.has_active_session && state.snapshot.is_none()
                && state.blocker == "runtime_session:no_active_session"
        }
}

/// Run a bounded sequence on one resident native owner. No detached request,
/// second scheduler, or profile publication. The caller owns the build
/// generation and must finalize or fail it after the scope returns.
pub(super) fn with_runtime<T>(
    generation: u64, deadline: Instant, task: &'static str,
    work: impl FnOnce(&dyn Fn(InferenceInput, &str) -> Result<InferenceOutput, String>) -> Result<T, String>,
) -> Result<T, String> {
    let permit = acquire_helper_task_permit(HelperTaskPriority::Diagnostic)
        .map_err(|_| "preview_scheduler_unavailable")?;
    let request_id = permit.request_id();
    let mut host = runtime().lock().map_err(|_| "preview_owner_unavailable")?;
    if !authority(generation, deadline) || host.has_meeting_resources()
        || host.active_request_id.is_some() || !host.release_idle_native_inference() {
        return Err("preview_owner_busy".into());
    }
    // Closing the idle owner may consume the cleanup budget. Never begin
    // a cold GPU open for a cancelled or expired voice operation.
    if !authority(generation, deadline) {
        return Err("preview_cancelled_or_timed_out".into());
    }
    let native = unsafe { NativeInferenceOwner::open_voice_candidate(None) }
        .map_err(|_| "preview_native_model_unavailable")?;
    host.native_inference = Some(native);
    host.active_task = Some(task.into());
    host.active_request_id = Some(request_id.into());
    let bridge_generation = host.generation_token;
    drop(host);

    let request = |input, failure: &str| -> Result<InferenceOutput, String> {
        let pending = {
            let mut host = runtime().lock().map_err(|_| "preview_owner_unavailable")?;
            if !authority(generation, deadline) || host.generation_token != bridge_generation
                || host.active_request_id.as_deref() != Some(request_id) || host.has_meeting_resources() {
                return Err("preview_cancelled_or_timed_out".into());
            }
            host.native_inference.as_mut().ok_or("preview_owner_unavailable")?
                .owner.begin(input, deadline).map_err(|_| "preview_native_request_failed")?
        };
        let output = pending.wait_output_with_cancel(|| !authority(generation, deadline))
            .map_err(|_| failure.to_string())?;
        if !authority(generation, deadline) {
            return Err("preview_cancelled_or_timed_out".into());
        }
        Ok(output)
    };
    let result = work(&request);

    // Keep the owner registered until request/thread release is verified.
    // Wait/drop of the pending request happens outside the lifecycle mutex.
    let mut host = runtime().lock().map_err(|_| "preview_owner_unavailable")?;
    if host.generation_token != bridge_generation || host.active_request_id.as_deref() != Some(request_id) {
        return Err("preview_owner_changed".into());
    }
    clear_active_request(&mut host, request_id);
    if !host.release_idle_native_inference() {
        set_blocked(&mut host, "Native voice operation cleanup requires retry.", "helper_bridge:cleanup_incomplete");
        return Err("preview_native_cleanup_incomplete".into());
    }
    drop(host);
    if !authority(generation, deadline) {
        return Err("preview_cancelled_or_timed_out".into());
    }
    result
}
