// Keep the public Meeting output API stable. RIFF/PCM validation is owned
// by the canonical decoder inside the runtime, so each delivery opens its
// WAV once rather than preflighting a path and reopening it for decoding.
#[path = "meeting_output_runtime.rs"]
mod runtime;

pub(crate) use runtime::MAX_DELIVERY_DEADLINE_MS;

pub use runtime::{
    cancel_meeting_output_for_generation, clear_prepared_meeting_output_device,
    deliver_meeting_output_wav, deliver_meeting_output_wav_for_source,
    prepare_meeting_output_device, probe_prepared_meeting_output_device_functionally,
    start_original_voice_output, stop_original_voice_output, original_voice_output_healthy,
    MeetingOutputDeliveryReport,
};
