use serde::Serialize;

use super::device_loss_guard::{device_loss_guard_for_meeting, DeviceLossGuardStatus};
use super::meeting_session::get_meeting_session_status;
use super::runtime_watchdog::{runtime_watchdog_for_meeting, RuntimeWatchdogStatus};

#[derive(Debug, Clone, Serialize)]
pub struct MeetingReliabilitySnapshot {
    pub watchdog: RuntimeWatchdogStatus,
    pub devices: DeviceLossGuardStatus,
}

#[tauri::command]
pub fn get_meeting_reliability_snapshot() -> MeetingReliabilitySnapshot {
    // One fresh observation per combined read; no cross-poll cache.
    let meeting = get_meeting_session_status();
    MeetingReliabilitySnapshot {
        watchdog: runtime_watchdog_for_meeting(&meeting),
        devices: device_loss_guard_for_meeting(&meeting),
    }
}
