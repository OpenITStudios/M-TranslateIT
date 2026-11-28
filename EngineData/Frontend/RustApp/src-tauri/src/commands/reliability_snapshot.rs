use serde::Serialize;

use super::device_loss_guard::{device_loss_guard_for_meeting, DeviceLossGuardStatus};
use super::meeting_session::get_meeting_session_status;
use super::long_session_health::{session_pressure_for_meeting, record_pressure_if_needed, SessionPressureStatus};
use super::runtime_watchdog::{runtime_watchdog_for_meeting, RuntimeWatchdogStatus};

#[derive(Debug, Clone, Serialize)]
pub struct MeetingReliabilitySnapshot {
    pub watchdog: RuntimeWatchdogStatus,
    pub devices: DeviceLossGuardStatus,
    pub pressure: SessionPressureStatus,
}

#[tauri::command]
pub fn get_meeting_reliability_snapshot() -> MeetingReliabilitySnapshot {
    // One fresh observation per combined read; no cross-poll cache.
    let meeting = get_meeting_session_status();
    let watchdog = runtime_watchdog_for_meeting(&meeting);
    let devices = device_loss_guard_for_meeting(&meeting);
    let pressure = session_pressure_for_meeting(&meeting);
    record_pressure_if_needed(&pressure);
    MeetingReliabilitySnapshot { watchdog, devices, pressure }
}
