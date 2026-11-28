use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};


mod scheduler;
#[cfg(all(test, windows))]
mod native_quality;

pub(crate) use scheduler::{
    acquire_helper_task_permit, acquire_scoped_helper_task_permit, wake_revoked_scheduler_waiters,
    HelperTaskPriority, SCHEDULER_SCOPE_REVOKED,
};
#[cfg(windows)]
pub(crate) use scheduler::{HelperTaskPermit, try_acquire_idle_diagnostic_permit};

pub const WORKER_CONTROL_RESPONSE_DEADLINE_MS: u128 = 5_000;
pub const WORKER_STATUS_RESPONSE_DEADLINE_MS: u128 = 30_000;
pub const WORKER_PRELOAD_RESPONSE_DEADLINE_MS: u128 = 120_000;
pub const WORKER_INFERENCE_RESPONSE_DEADLINE_MS: u128 = 90_000;
// Standalone translation is a single native request with its own bounded budget.
pub const STANDALONE_TRANSLATION_DEADLINE_MS: u128 = 180_000;
pub const WORKER_SYNTHESIS_RESPONSE_DEADLINE_MS: u128 = 45_000;
pub const WORKER_FALLBACK_RESPONSE_DEADLINE_MS: u128 = WORKER_STATUS_RESPONSE_DEADLINE_MS;
pub fn worker_response_deadline_ms(task: &str) -> u128 {
    match task {
        "ping" => WORKER_CONTROL_RESPONSE_DEADLINE_MS,
        "status" => WORKER_STATUS_RESPONSE_DEADLINE_MS,
        "asr_preload" | "translation_preload" | "voice_actor_preflight" => {
            WORKER_PRELOAD_RESPONSE_DEADLINE_MS
        }
        "transcribe" | "translate" => WORKER_INFERENCE_RESPONSE_DEADLINE_MS,
        "voice_actor_synthesize" => WORKER_SYNTHESIS_RESPONSE_DEADLINE_MS,
        _ => WORKER_FALLBACK_RESPONSE_DEADLINE_MS,
    }
}

// Only standalone Text translation receives the extended multi-chunk budget. Meeting
// lanes remain on the bounded inference deadline so optional incoming work cannot hold
// the single shared helper pipeline for the standalone Text budget while required
// outbound work is waiting.
pub fn worker_response_deadline_for_priority(task: &str, priority: HelperTaskPriority) -> u128 {
    if task == "translate" && priority == HelperTaskPriority::Text {
        return STANDALONE_TRANSLATION_DEADLINE_MS;
    }
    worker_response_deadline_ms(task)
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(Default))]
pub struct HelperBridgeStatus {
    pub state: String,
    pub message: String,
    pub cuda_ready: bool,
    pub provider_ready: bool,
    pub functional_outbound_ready: bool,
    pub functional_outbound_verified_unix_ms: Option<u128>,
    pub degraded_mode: bool,
    pub active_task: Option<String>,
    pub active_request_id: Option<String>,
    pub active_meeting_generation: Option<u64>,
    pub active_meeting_session_id: Option<String>,
    pub active_meeting_lane: Option<String>,
    pub generation_token: u64,
    pub last_error: Option<String>,
    pub stderr_log_path: Option<String>,
    pub updated_unix_ms: u128,
    pub runtime_claim: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HelperBridgeActionResult {
    pub ok: bool,
    pub state: String,
    pub message: String,
    pub generation_token: u64,
    pub runtime_claim: String,
}

// The resident native pair belongs to the existing helper lifecycle.
// Its session identity persists between inference requests.
#[cfg(windows)]
pub(super) struct NativeInferenceOwner {
    pub(super) session_id: Option<String>,
    pub(super) owner: translateit_native_cuda::RuntimeOwner,
    // A candidate binding is only meaningful for the exact prepared scope.
    native_voice: Option<NativeVoiceSession>,
}

#[cfg(windows)]
struct NativeVoiceSession {
    generation: Option<u64>,
    voice_id: String,
    actor_token: String,
    binding: translateit_native_cuda::VoiceBinding,
}

#[cfg(windows)]
impl NativeInferenceOwner {
    fn asset_paths(paths: &crate::engine::paths::ProjectPaths) -> Result<(PathBuf, PathBuf, PathBuf, PathBuf, &'static str), translateit_native_cuda::Error> {
        use translateit_native_cuda::Error;
        let pins = option_env!("TRANSLATEIT_NATIVE_DLL_PINS")
            .filter(|value| !value.is_empty()).ok_or(Error::LibraryIdentityMismatch)?;
        if !paths.packaged_context_initialized && !paths.development_root_verified {
            return Err(Error::InvalidInput);
        }
        let assets = PathBuf::from(&paths.runtime_root).join("EngineData/Backend/RuntimeAssets");
        Ok((
            assets.join("NativeCuda/bin/translateit_native_cuda.dll"),
            PathBuf::from(&paths.asr_model_dir).join("qwen3-asr-1.7b/Qwen3-ASR-1.7B-Q8_0.gguf"),
            PathBuf::from(&paths.asr_model_dir).join("qwen3-asr-1.7b/mmproj-Qwen3-ASR-1.7B-Q8_0.gguf"),
            PathBuf::from(&paths.translation_model_dir).join("hy-mt2-1.8b/Hy-MT2-1.8B-Q6_K.gguf"),
            pins,
        ))
    }

    pub(super) fn assets_present() -> bool {
        Self::assets_present_at(&crate::engine::paths::ProjectPaths::discover())
    }

    fn assets_present_at(paths: &crate::engine::paths::ProjectPaths) -> bool {
        Self::asset_paths(paths).is_ok_and(|(dll, asr, projector, mt, _)| {
            [dll, asr, projector, mt].iter().all(|path| {
                std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() > 0)
            })
        })
    }

    /// The package/build owner must authenticate the pins and establish a
    /// controlled runtime root before this lifecycle admission is used.
    /// A runtime manifest, arbitrary PATH or user-provided PCI index is not
    /// an authority. Build-time pins come from the reviewed DLL closure.
    ///
    /// # Safety
    /// The caller must establish trusted parent directories and the reviewed
    /// Windows system dependency closure. No untrusted private DLL loader
    /// may race this admission.
    pub(super) unsafe fn open(session_id: Option<String>) -> Result<Self, translateit_native_cuda::Error> {
        use translateit_native_cuda::RuntimeOwner;
        let paths = crate::engine::paths::ProjectPaths::discover();
        let (dll, asr, projector, mt, pins) = Self::asset_paths(&paths)?;
        let owner = unsafe { RuntimeOwner::open(&dll, pins, &asr, &projector, &mt, None)? };
        Ok(Self { session_id, owner, native_voice: None })
    }

    fn voice_asset_paths(paths: &crate::engine::paths::ProjectPaths) -> (PathBuf, PathBuf) {
        let voice = PathBuf::from(&paths.voice_runtime_dir).join("ModelData/zonos3-mini");
        (voice.join("ZONOS3-Mini-Q8_0.gguf"), voice.join("mmproj-ZONOS3-Mini-F32.gguf"))
    }

    fn voice_assets_present(paths: &crate::engine::paths::ProjectPaths) -> bool {
        use std::os::windows::fs::MetadataExt;
        let (model, codec) = Self::voice_asset_paths(paths);
        [(model, 854726208_u64), (codec, 1142502880_u64)].iter().all(|(path, bytes)| {
            std::fs::symlink_metadata(path).is_ok_and(|meta| {
                meta.is_file() && meta.file_attributes() & 0x400 == 0 && meta.len() == *bytes
            })
        })
    }

    /// Candidate open, used only by explicit selected-voice preparation,
    /// not by live synthesis or the current pair-only production admission.
    /// All model paths are derived from the controlled runtime root.
    ///
    /// # Safety
    /// Same reviewed build/closure and trusted root requirements as open.
    pub(super) unsafe fn open_voice_candidate(session_id: Option<String>)
        -> Result<Self, translateit_native_cuda::Error>
    {
        use translateit_native_cuda::{Error, RuntimeOwner};
        if session_id.as_deref() == Some("") { return Err(Error::InvalidInput); }
        let paths = crate::engine::paths::ProjectPaths::discover();
        let (dll, asr, projector, mt, pins) = Self::asset_paths(&paths)?;
        let (model, codec) = Self::voice_asset_paths(&paths);
        let owner = unsafe {
            RuntimeOwner::open_with_voice(&dll, pins, &asr, &projector, &mt, None,
                Some((&model, &codec)))?
        };
        Ok(Self { session_id, owner, native_voice: None })
    }

    fn voice_scope_matches(&self, generation: Option<u64>, session_id: Option<&str>, preparing: bool) -> bool {
        use crate::engine::runtime_state::{latest_runtime_session_state, APPLICATION_MEETING_OWNER_ID};
        if self.session_id.as_deref() != session_id { return false; }
        let state = latest_runtime_session_state();
        match (generation, session_id) {
            (None, None) => preparing && state.snapshot.is_none() && !state.has_active_session
                && state.blocker == "runtime_session:no_active_session",
            (Some(generation), Some(session_id)) if generation != 0 => state.snapshot.is_some_and(|snapshot| {
                snapshot.owner_id == APPLICATION_MEETING_OWNER_ID && snapshot.authority_active
                    && snapshot.generation == generation && snapshot.session_id == session_id
                    && (if preparing { matches!(snapshot.phase.as_str(), "starting" | "live") }
                        else { snapshot.phase == "live" })
            }),
            _ => false,
        }
    }

    /// Call only under the existing helper permit and lifecycle owner.
    /// The returned request must be awaited OUTSIDE the lifecycle mutex.
    pub(super) fn begin_selected_voice(
        &mut self, generation: Option<u64>, session_id: Option<&str>,
        reference: crate::commands::voice_lab::NativeSelectedReference, deadline: Instant,
    ) -> Result<(String, translateit_native_cuda::PendingInference), translateit_native_cuda::Error> {
        use translateit_native_cuda::{Error, InferenceInput};
        if !self.voice_scope_matches(generation, session_id, true) {
            return Err(Error::InvalidInput);
        }
        self.native_voice = None;
        let (voice_id, pcm) = reference.into_parts();
        let pending = self.owner.begin(InferenceInput::BindVoice { reference: pcm }, deadline)?;
        Ok((voice_id, pending))
    }

    /// Publish only the typed receipt after the transport verifies its
    /// request ID and helper generation, and the same Meeting scope again.
    pub(super) fn finish_selected_voice(
        &mut self, generation: Option<u64>, session_id: Option<&str>, voice_id: &str,
        request_id: &str, output: translateit_native_cuda::InferenceOutput,
    ) -> Result<String, translateit_native_cuda::Error> {
        use translateit_native_cuda::{Error, InferenceOutput};
        self.native_voice = None;
        if !self.voice_scope_matches(generation, session_id, true)
            || !crate::commands::voice_lab::selected_native_identity(&crate::engine::paths::ProjectPaths::discover())
                .is_ok_and(|selected| selected == voice_id) {
            return Err(Error::InvalidInput);
        }
        let InferenceOutput::VoiceBound(binding) = output else { return Err(Error::InvalidReceipt); };
        let actor_token = format!("zonos3:{voice_id}:{request_id}");
        self.native_voice = Some(NativeVoiceSession { generation, voice_id: voice_id.to_owned(), actor_token: actor_token.clone(), binding });
        Ok(actor_token)
    }

    /// Only the selected binding for the exact live Meeting may synthesize.
    /// The scheduler permit and transport retain ownership through reply.
    pub(super) fn begin_bound_voice(
        &mut self, generation: Option<u64>, session_id: Option<&str>, actor_token: &str,
        preparing: bool, text: String, deadline: Instant,
    ) -> Result<translateit_native_cuda::PendingInference, translateit_native_cuda::Error> {
        use translateit_native_cuda::{Error, InferenceInput};
        if !self.voice_scope_matches(generation, session_id, preparing) { return Err(Error::InvalidInput); }
        let selected = self.native_voice.as_ref()
            .filter(|voice| voice.generation == generation && voice.actor_token == actor_token)
            .ok_or(Error::InvalidInput)?;
        self.owner.begin(InferenceInput::Voice { text, binding: selected.binding }, deadline)
    }
}

pub struct HelperBridgeRuntime {
    #[cfg(windows)]
    pub(super) native_inference: Option<NativeInferenceOwner>,
    pub state: String,
    pub message: String,
    pub cuda_ready: bool,
    pub provider_ready: bool,
    pub degraded_mode: bool,
    pub active_task: Option<String>,
    pub active_request_id: Option<String>,
    pub active_meeting_generation: Option<u64>,
    pub active_meeting_session_id: Option<String>,
    pub active_meeting_lane: Option<String>,
    pub generation_token: u64,
    pub last_error: Option<String>,
    // Keep exact Meeting cleanup attribution until its owner verifies release.
    pub pending_meeting_cleanup_session_id: Option<String>,
    pub updated_unix_ms: u128,
}

impl Default for HelperBridgeRuntime {
    fn default() -> Self {
        Self {
            #[cfg(windows)]
            native_inference: None,
            state: "not_started".to_string(),
            message: "Native helper lifecycle is not initialized."
                .to_string(),
            cuda_ready: false,
            provider_ready: false,
            degraded_mode: false,
            active_task: None,
            active_request_id: None,
            active_meeting_generation: None,
            active_meeting_session_id: None,
            active_meeting_lane: None,
            generation_token: 0,
            last_error: None,
            pending_meeting_cleanup_session_id: None,
            updated_unix_ms: unix_ms(),
        }
    }
}

impl HelperBridgeRuntime {
    // Ready denotes initialized Rust lifecycle, not loaded models or successful inference.
    pub(super) fn native_lifecycle_ready(&self) -> bool {
        self.state == "ready" && self.generation_token != 0
            && self.pending_meeting_cleanup_session_id.is_none()
            && self.last_error.as_deref() != Some("helper_bridge:cleanup_incomplete")
    }

    pub(super) fn native_owner_healthy(&self) -> bool {
        #[cfg(windows)]
        { self.native_inference.as_ref().is_none_or(|native| native.owner.is_available()) }
        #[cfg(not(windows))]
        { false }
    }

    // Cold (no owner) is eligible for explicit preparation; a retained
    // closed/failed owner is not. Both startup and status use this same
    // state projection without reloading models or probing the GPU.
    pub(super) fn effective_state(&self, native_available: bool) -> &str {
        if self.state == "ready" && (!native_available
            || !self.native_lifecycle_ready())
        {
            "blocked"
        } else {
            self.state.as_str()
        }
    }

    pub(super) fn native_meeting_session_id(&self) -> Option<&str> {
        #[cfg(windows)]
        {
            self.native_inference.as_ref().and_then(|native| native.session_id.as_deref())
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    pub(super) fn has_meeting_resources(&self) -> bool {
        self.pending_meeting_cleanup_session_id.is_some()
            || self.active_meeting_session_id.is_some()
            || self.native_meeting_session_id().is_some()
    }

    // No Meeting owner may be released by idle diagnostic/voice admission.
    pub(super) fn release_idle_native_inference(&mut self) -> bool {
        if self.native_meeting_session_id().is_some() || self.active_request_id.is_some() {
            return false;
        }
        let started = Instant::now();
        while !self.try_close_native_inference() {
            if started.elapsed() >= HELPER_CLEANUP_BUDGET { return false; }
            thread::sleep(Duration::from_millis(10));
        }
        self.cuda_ready = false;
        true
    }

    fn try_close_native_inference(&mut self) -> bool {
        #[cfg(windows)]
        {
            if let Some(native) = self.native_inference.as_mut() {
                if native.owner.try_close() != Ok(true) {
                    return false;
                }
                // Keep a closed Meeting-owned slot until exact Stop verifies
                // full lifecycle cleanup; no polling or restart may erase it.
                if native.session_id.is_none() {
                    self.native_inference = None;
                }
            }
        }
        true
    }

    // Call only after stop_child has verified all resource cleanup and the
    // existing lifecycle has confirmed the matching Meeting or idle orphan.
    pub(super) fn release_closed_native_meeting_owner(&mut self) {
        #[cfg(windows)]
        {
            self.native_inference = None;
        }
    }
}

static HELPER_BRIDGE_RUNTIME: OnceLock<Mutex<HelperBridgeRuntime>> = OnceLock::new();

pub fn runtime() -> &'static Mutex<HelperBridgeRuntime> {
    HELPER_BRIDGE_RUNTIME.get_or_init(|| Mutex::new(HelperBridgeRuntime::default()))
}

pub fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub fn runtime_claim(runtime: &HelperBridgeRuntime) -> String {
    if runtime.native_lifecycle_ready() && runtime.native_owner_healthy() {
        "native_rust_lifecycle_initialized".to_string()
    } else {
        "native_rust_lifecycle_not_ready".to_string()
    }
}

pub fn status_from_runtime(runtime: &HelperBridgeRuntime) -> HelperBridgeStatus {
    let native_available = !cfg!(windows) || runtime.native_owner_healthy();
    HelperBridgeStatus {
        state: runtime.effective_state(native_available).to_owned(),
        message: runtime.message.clone(),
        cuda_ready: runtime.cuda_ready && native_available,
        provider_ready: runtime.provider_ready && native_available,
        functional_outbound_ready: false,
        functional_outbound_verified_unix_ms: None,
        degraded_mode: runtime.degraded_mode,
        active_task: runtime.active_task.clone(),
        active_request_id: runtime.active_request_id.clone(),
        active_meeting_generation: runtime.active_meeting_generation,
        active_meeting_session_id: runtime.active_meeting_session_id.clone(),
        active_meeting_lane: runtime.active_meeting_lane.clone(),
        generation_token: runtime.generation_token,
        last_error: runtime.last_error.clone(),
        stderr_log_path: None,
        updated_unix_ms: runtime.updated_unix_ms,
        runtime_claim: runtime_claim(runtime),
    }
}

pub fn action_result(ok: bool, runtime: &HelperBridgeRuntime) -> HelperBridgeActionResult {
    HelperBridgeActionResult {
        ok,
        state: runtime.state.clone(),
        message: runtime.message.clone(),
        generation_token: runtime.generation_token,
        runtime_claim: runtime_claim(runtime),
    }
}

pub fn clear_active_request(runtime: &mut HelperBridgeRuntime, request_id: &str) {
    if runtime.active_request_id.as_deref() == Some(request_id) {
        runtime.active_task = None;
        runtime.active_request_id = None;
        runtime.active_meeting_generation = None;
        runtime.active_meeting_session_id = None;
        runtime.active_meeting_lane = None;
    }
}

pub fn set_blocked(
    runtime: &mut HelperBridgeRuntime,
    message: &str,
    error: &str,
) -> HelperBridgeActionResult {
    runtime.state = "blocked".to_string();
    runtime.message = message.to_string();
    runtime.cuda_ready = false;
    runtime.provider_ready = false;
    runtime.degraded_mode = false;
    runtime.active_task = None;
    runtime.active_request_id = None;
    runtime.active_meeting_generation = None;
    runtime.active_meeting_session_id = None;
    runtime.active_meeting_lane = None;
    runtime.last_error = Some(error.to_string());
    runtime.updated_unix_ms = unix_ms();
    action_result(false, runtime)
}

pub fn worker_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub fn worker_text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

struct CandidateReadiness {
    pair_ready: bool,
    voice_ready: bool,
    ready: bool,
    resident: bool,
    bound: bool,
    blocker: &'static str,
    note: &'static str,
}

fn candidate_readiness(lifecycle_ready: bool, pair_present: bool,
                       voice_assets_present: bool, selected: Result<&str, &'static str>,
                       has_pci: bool, has_bound_token: bool) -> CandidateReadiness {
    let pair_ready = lifecycle_ready && pair_present;
    let voice_ready = lifecycle_ready && voice_assets_present && selected.is_ok();
    let ready = pair_ready && voice_ready;
    let resident = pair_ready && has_pci;
    let bound = ready && resident && has_bound_token;
    let blocker = if !lifecycle_ready { "native_inference:lifecycle_not_ready" }
        else if !pair_present { "native_inference:assets_or_owner_unavailable" }
        else if !voice_assets_present { "native_voice:model_assets_unavailable" }
        else { selected.err().unwrap_or("") };
    let note = if ready && bound { "Native model owner and selected voice are bound; functional inference proof is separate." }
        else if ready { "Native assets and selected reference are eligible; explicit preparation must verify hashes, CUDA and inference." }
        else { "Native runtime prerequisites are unavailable." };
    CandidateReadiness { pair_ready, voice_ready, ready, resident, bound, blocker, note }
}

fn candidate_status_payload(readiness: &CandidateReadiness, pci: Option<&str>, bound_token: Option<&str>) -> Value {
    use super::helper_bridge::{NATIVE_ASR_MODEL, NATIVE_MT_MODEL};
    let CandidateReadiness { pair_ready, voice_ready, ready, resident, bound, blocker, note } = *readiness;
    let device = if resident { "cuda" } else if pair_ready { "not_loaded" } else { "blocked" };
    json!({
        "ok": ready, "stage": "local_realtime_worker_preflight",
        "runtime_backend": "native_cuda", "blocker": blocker, "provider_ready": ready,
        "note": note,
        "readiness": {
            "asr": pair_ready, "translation_id_en": pair_ready,
            "translation_en_id": pair_ready, "translation_bidirectional": pair_ready,
            "voice_actor_tts": voice_ready, "cuda_degraded": false,
        },
        "cuda_pci_id": pci.filter(|_| resident), "cuda_capability_known": resident,
        "cpu_fallback_active": false, "selected_device": device, "selected_translation_device": device,
        "selected_compute_type": if resident { "q8_0" } else { "blocked" },
        "asr_primary_model_ready": pair_ready, "asr_model_ready": pair_ready,
        "asr_active_model_id": NATIVE_ASR_MODEL,
        "translation_id_en_ready": pair_ready, "translation_en_id_ready": pair_ready,
        "translation_bidirectional_ready": pair_ready, "voice_actor_ready": voice_ready,
        "voice_actor_token": bound_token.filter(|_| bound),
        "tts": {"model_id": "zonos3-mini-q8_0", "sample_rate_hz": 44100},
        "dependencies": {"python_required": false, "torch_required": false},
        "gpu": {},
        "loaded": {
            "asr": resident, "asr_model_id": NATIVE_ASR_MODEL,
            "asr_device": device, "asr_compute_type": if resident { "q8_0" } else { "blocked" },
            "translation_model_id": NATIVE_MT_MODEL,
            "translation_directions": if resident { &["id->en", "en->id"][..] } else { &[][..] },
            "voice_actor": bound, "voice_actor_device": if bound { "cuda" } else { "not_loaded" },
        },
    })
}

#[cfg(windows)]
pub(super) fn project_candidate_status(runtime: &mut HelperBridgeRuntime, include_payload: bool) -> Option<Value> {
    let owner = runtime.native_inference.as_ref().filter(|native| native.owner.is_available());
    let admitted = runtime.native_lifecycle_ready()
        && (runtime.native_inference.is_none() || owner.is_some())
        && runtime.active_request_id.is_none();
    // An unavailable lifecycle cannot admit any resource. Otherwise use one
    // fresh path observation for every presence and selected-voice check.
    let (pair, voice, selected) = if admitted {
        let paths = crate::engine::paths::ProjectPaths::discover();
        (NativeInferenceOwner::assets_present_at(&paths),
            NativeInferenceOwner::voice_assets_present(&paths),
            crate::commands::voice_lab::selected_native_identity(&paths))
    } else { (false, false, Err("native_inference:lifecycle_not_ready")) };
    let binding = owner.and_then(|native| native.native_voice.as_ref());
    let selected = selected.as_deref();
    let selected = if binding.is_some_and(|bound| selected.is_ok_and(|id| id != bound.voice_id.as_str())) {
        Err("native_voice:selected_reference_changed")
    } else { selected.map_err(|error| *error) };
    let pci = owner.map(|native| native.owner.device_id());
    let token = binding.filter(|bound| selected.is_ok_and(|id| id == bound.voice_id.as_str()))
        .map(|bound| bound.actor_token.as_str());
    let readiness = candidate_readiness(admitted, pair, voice, selected, pci.is_some(), token.is_some());
    // Start needs only the state update, not a diagnostic JSON document.
    let projected = include_payload.then(|| candidate_status_payload(&readiness, pci, token));
    runtime.cuda_ready = readiness.resident;
    runtime.provider_ready = readiness.ready;
    runtime.degraded_mode = false;
    if runtime.last_error.as_deref() != Some("helper_bridge:cleanup_incomplete") {
        runtime.last_error = (!readiness.blocker.is_empty()).then(|| readiness.blocker.to_owned());
    }
    runtime.message = readiness.note.to_owned();
    runtime.updated_unix_ms = unix_ms();
    projected
}

pub fn apply_worker_response(runtime: &mut HelperBridgeRuntime, value: &Value) -> bool {
    let ok = worker_bool(value, "ok");
    let stage = value
        .get("stage")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if stage == "local_realtime_worker_preflight" {
        // Only project_candidate_status owns capability projection.
        return false;
    } else {
        let blocker = worker_text(value, "blocker").unwrap_or_default();
        let hard_voice_actor_failure = stage == "voice_actor_synthesize"
            && !matches!(
                blocker.as_str(),
                "voice_actor:empty_text" | "voice_actor:text_too_large"
            );
        let required_outbound_prepare_failed = !ok
            && (stage == "asr_preload"
                || stage == "voice_actor_preflight"
                || hard_voice_actor_failure
                || (stage == "translation_preload"
                    && value.get("direction_pair").and_then(Value::as_str) == Some("id->en")));
        if required_outbound_prepare_failed {
            // A required Start-preparation stage has proved the current outbound
            // provider unusable. Keep readiness fail-closed until a later status or
            // successful preparation re-establishes it.
            runtime.provider_ready = false;
        }

        runtime.last_error = if ok {
            None
        } else {
            worker_text(value, "blocker").filter(|blocker| !blocker.is_empty())
        };
        runtime.message = worker_text(value, "note")
            .or_else(|| worker_text(value, "stage"))
            .unwrap_or_else(|| {
                if ok {
                    "Helper request completed.".to_string()
                } else {
                    "Native request failed; lifecycle readiness is separate."
                        .to_string()
                }
            });
    }

    runtime.updated_unix_ms = unix_ms();
    ok
}

const HELPER_CLEANUP_BUDGET: Duration = Duration::from_millis(250);

/// Retain exact Meeting attribution and the native owner if closure is not
/// verified within the existing bounded budget. Only the owning Stop may
/// release the attribution, even after the native runtime has closed.
pub fn stop_child(runtime: &mut HelperBridgeRuntime) -> bool {
    if runtime.pending_meeting_cleanup_session_id.is_none() {
        runtime.pending_meeting_cleanup_session_id = runtime.active_meeting_session_id.clone()
            .or_else(|| runtime.native_meeting_session_id().map(str::to_owned));
    }
    let started = Instant::now();
    while !runtime.try_close_native_inference() {
        if started.elapsed() >= HELPER_CLEANUP_BUDGET { return false; }
        thread::sleep(Duration::from_millis(10));
    }
    true
}

#[cfg(test)]
mod readiness_contract_tests {
    use super::*;

    #[test]
    fn retained_unavailable_native_owner_blocks_ready_reuse() {
        // Pure availability input, not a synthetic native owner or CUDA proof.
        let mut runtime = HelperBridgeRuntime {
            state: "ready".into(),
            generation_token: 1,
            ..HelperBridgeRuntime::default()
        };
        assert_eq!(runtime.effective_state(true), "ready");
        assert_eq!(runtime.effective_state(false), "blocked");
        runtime.state = "stopped".into();
        assert_eq!(runtime.effective_state(false), "stopped");
        runtime.state = "blocked".into();
        assert_eq!(runtime.effective_state(true), "blocked");
    }

    fn status_payload(lifecycle_ready: bool, pair_present: bool, voice_present: bool,
                      selected: Result<&str, &'static str>, pci: Option<&str>, token: Option<&str>) -> Value {
        let readiness = candidate_readiness(lifecycle_ready, pair_present, voice_present, selected, pci.is_some(), token.is_some());
        let payload = candidate_status_payload(&readiness, pci, token);
        assert_eq!(payload["ok"], readiness.ready);
        assert_eq!(payload["provider_ready"], readiness.ready);
        assert_eq!(payload["cuda_capability_known"], readiness.resident);
        assert_eq!(payload["blocker"], readiness.blocker);
        assert_eq!(payload["note"], readiness.note);
        payload
    }

    #[test]
    fn candidate_status_separates_lifecycle_asset_presence_and_voice_binding() {
        let stopped = status_payload(false, true, true, Ok("MaleVoice"), None, None);
        assert_eq!(stopped["provider_ready"], false);
        let cold = status_payload(true, true, true, Ok("MaleVoice"), None, None);
        assert_eq!(cold["provider_ready"], true);
        assert_eq!(cold["loaded"]["asr"], false);
        assert_eq!(cold["loaded"]["voice_actor"], false);
        assert_eq!(cold["cuda_capability_known"], false);
        assert_eq!(cold["voice_actor_token"], Value::Null);
        assert_eq!(cold["dependencies"]["python_required"], false);
        let pair = status_payload(true, true, true, Ok("MaleVoice"),
            Some("0000:01:00.0"), None);
        assert_eq!(pair["loaded"]["asr"], true);
        assert_eq!(pair["loaded"]["voice_actor"], false);
        let bound = status_payload(true, true, true, Ok("MaleVoice"),
            Some("0000:01:00.0"), Some("bound"));
        assert_eq!(bound["loaded"]["voice_actor"], true);
        let missing = status_payload(true, true, false, Ok("MaleVoice"), None, None);
        assert_eq!(missing["provider_ready"], false);
        assert_eq!(missing["readiness"]["translation_id_en"], true);
        let personal = status_payload(true, true, true,
            Err("native_voice:personal_clone_approval_required"), None, None);
        assert_eq!(personal["provider_ready"], false);
        assert_eq!(personal["blocker"], "native_voice:personal_clone_approval_required");
    }

}

#[cfg(test)]
mod deadline_policy_tests {
    use super::*;

    #[test]
    fn worker_deadlines_follow_bounded_task_cost_classes() {
        assert_eq!(worker_response_deadline_ms("ping"), 5_000);
        assert_eq!(worker_response_deadline_ms("status"), 30_000);
        assert_eq!(worker_response_deadline_ms("tts_preflight"), 30_000);
        assert_eq!(worker_response_deadline_ms("asr_preload"), 120_000);
        assert_eq!(worker_response_deadline_ms("translation_preload"), 120_000);
        assert_eq!(worker_response_deadline_ms("transcribe"), 90_000);
        assert_eq!(worker_response_deadline_ms("translate"), 90_000);
        assert_eq!(worker_response_deadline_ms("voice_actor_synthesize"), 45_000);
        assert_eq!(worker_response_deadline_ms("unknown"), 30_000);
    }

    #[test]
    fn standalone_translation_deadline_applies_only_to_text_priority() {
        assert_eq!(STANDALONE_TRANSLATION_DEADLINE_MS, 180_000);
        assert!(
            STANDALONE_TRANSLATION_DEADLINE_MS > WORKER_INFERENCE_RESPONSE_DEADLINE_MS,
            "standalone translate must get a strictly larger single-attempt budget"
        );
        assert_eq!(
            worker_response_deadline_for_priority(
                "translate",
                HelperTaskPriority::MeetingOutbound
            ),
            WORKER_INFERENCE_RESPONSE_DEADLINE_MS
        );
        assert_eq!(
            worker_response_deadline_for_priority(
                "translate",
                HelperTaskPriority::MeetingIncoming
            ),
            WORKER_INFERENCE_RESPONSE_DEADLINE_MS
        );
        assert_eq!(
            worker_response_deadline_for_priority("translate", HelperTaskPriority::Text),
            STANDALONE_TRANSLATION_DEADLINE_MS
        );
        assert_eq!(
            worker_response_deadline_for_priority("translate", HelperTaskPriority::Diagnostic),
            WORKER_INFERENCE_RESPONSE_DEADLINE_MS
        );
        assert_eq!(
            worker_response_deadline_for_priority("transcribe", HelperTaskPriority::Text),
            WORKER_INFERENCE_RESPONSE_DEADLINE_MS
        );
        assert_eq!(
            worker_response_deadline_for_priority("unknown", HelperTaskPriority::Diagnostic),
            WORKER_FALLBACK_RESPONSE_DEADLINE_MS
        );
    }
}
