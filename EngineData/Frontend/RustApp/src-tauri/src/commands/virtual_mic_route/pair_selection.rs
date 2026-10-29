//! Pure selection of the currently admitted Windows Meeting render/capture pair.
//! No device handles, persistent state, driver installation or Meeting lifecycle.
use crate::commands::audio::{AudioDeviceListReport, AudioDeviceSummary};

#[derive(Debug, Clone)]
struct RoutePairCandidate {
    pair_id: String,
    output_device: String,
    input_device: String,
}

#[derive(Debug, Clone)]
pub(super) struct RouteSelection {
    pub(super) route_pair_id: Option<String>,
    pub(super) selected_output_device: Option<String>,
    pub(super) selected_input_device: Option<String>,
    pub(super) output_device_found: bool,
    pub(super) input_device_found: bool,
    pub(super) blocker: String,
}

const TRANSLATEIT_RENDER_ENDPOINT: &str = "TranslateIT Microphone Feed";
const TRANSLATEIT_CAPTURE_ENDPOINT: &str = "TranslateIT Meeting Microphone";
// The unaccepted paired-source prototype can remain on a developer machine.
// It is not a supported provider, nor the approved capture-only target.
// Names are diagnostics only; a name must never confer driver trust.

fn supported_virtual_pair_provider(name: &str) -> bool {
    // The current packaged route is the *standard* VB-CABLE, not VB-CABLE
    // A+B/C+D, Voicemeeter's mixer buses, or a different virtual-audio driver.
    // Those providers have distinct drivers/topologies and must never become
    // an implicit fallback simply because a playback/recording name matches.
    let normalized = name.to_ascii_lowercase();
    normalized.contains("vb-audio virtual cable")
        && (normalized.starts_with("cable input")
            || normalized.starts_with("cable output"))
}

fn endpoint_pair_identity(name: &str, expected_role: &str) -> Option<String> {
    if !supported_virtual_pair_provider(name) {
        return None;
    }
    let normalized = name.to_ascii_lowercase();
    let tokens = normalized
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if !tokens.contains(&expected_role) {
        return None;
    }
    let identity = tokens
        .into_iter()
        .filter(|token| !matches!(*token, "input" | "output"))
        .collect::<Vec<_>>()
        .join("-");
    (!identity.is_empty()).then_some(identity)
}

pub(super) fn matched_pair_identity(output_device: &str, input_device: &str) -> Option<String> {
    // Virtual-cable naming is role-inverted from TranslateIT's point of view:
    // TTS is rendered to the cable playback-side "Input" endpoint and the meeting
    // application selects the paired recording-side "Output" endpoint as microphone.
    let output_identity = endpoint_pair_identity(output_device, "input")?;
    let input_identity = endpoint_pair_identity(input_device, "output")?;
    (output_identity == input_identity).then(|| format!("windows_virtual_pair:{output_identity}"))
}

fn matched_pair_candidates(
    output_devices: &[AudioDeviceSummary],
    input_devices: &[AudioDeviceSummary],
) -> Vec<RoutePairCandidate> {
    let mut candidates = Vec::new();
    for output in output_devices {
        for input in input_devices {
            let Some(pair_id) = matched_pair_identity(&output.name, &input.name) else {
                continue;
            };
            if candidates
                .iter()
                .any(|candidate: &RoutePairCandidate| candidate.pair_id == pair_id)
            {
                continue;
            }
            candidates.push(RoutePairCandidate {
                pair_id,
                output_device: output.name.clone(),
                input_device: input.name.clone(),
            });
        }
    }
    candidates.sort_by(|left, right| left.pair_id.cmp(&right.pair_id));
    candidates
}

// Discovery and actual stream preparation must agree. A name collision
// cannot be used to claim a ready matched route.
pub(super) fn reject_ambiguous_pair(
    mut selection: RouteSelection,
    devices: &AudioDeviceListReport,
) -> RouteSelection {
    let output_collision = selection
        .selected_output_device
        .as_ref()
        .is_some_and(|name| devices.ambiguous_output_names.contains(name));
    let input_collision = selection
        .selected_input_device
        .as_ref()
        .is_some_and(|name| devices.ambiguous_input_names.contains(name));
    if output_collision || input_collision {
        selection.blocker = "virtual_mic:matched_route_pair_ambiguous".to_string();
    }
    selection
}

fn primary_vb_cable_pair(candidate: &RoutePairCandidate) -> bool {
    let output = candidate.output_device.to_ascii_lowercase();
    let input = candidate.input_device.to_ascii_lowercase();
    output.starts_with("cable input")
        && input.starts_with("cable output")
        && output.contains("vb-audio virtual cable")
        && input.contains("vb-audio virtual cable")
}

pub(super) fn select_route_pair(
    output_devices: &[AudioDeviceSummary],
    input_devices: &[AudioDeviceSummary],
) -> RouteSelection {
    // Only the standard packaged VB-CABLE pair is admitted. The native
    // paired-source prototype does not have a signed/installed/audio-accepted
    // route, and the approved capture-only design needs a different transport.
    let candidates = matched_pair_candidates(output_devices, input_devices);
    let primary = candidates
        .iter()
        .filter(|candidate| primary_vb_cable_pair(candidate))
        .collect::<Vec<_>>();
    // Only the standard VB-CABLE pair is automatically supported.
    // Never activate a different mixer/cable if the installed standard pair is
    // absent or ambiguous.
    let selected = if primary.len() == 1 {
        primary.first().copied()
    } else {
        None
    };

    if let Some(candidate) = selected {
        return RouteSelection {
            route_pair_id: Some(candidate.pair_id.clone()),
            selected_output_device: Some(candidate.output_device.clone()),
            selected_input_device: Some(candidate.input_device.clone()),
            output_device_found: true,
            input_device_found: true,
            blocker: String::new(),
        };
    }

    RouteSelection {
        route_pair_id: None,
        selected_output_device: None,
        selected_input_device: None,
        output_device_found: false,
        input_device_found: false,
        blocker: if !primary.is_empty() {
            "virtual_mic:matched_route_pair_ambiguous".to_string()
        } else if output_devices.iter().any(|d| d.name == TRANSLATEIT_RENDER_ENDPOINT)
            || input_devices.iter().any(|d| d.name == TRANSLATEIT_CAPTURE_ENDPOINT)
        {
            // Do not misreport an experimental/partial driver as a ready route.
            "virtual_mic:unaccepted_native_candidate".to_string()
        } else {
            "virtual_mic:matched_route_pair_missing".to_string()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{matched_pair_identity, reject_ambiguous_pair, select_route_pair};
    use crate::commands::audio::{AudioDeviceListReport, AudioDeviceSummary};

    fn device(name: &str) -> AudioDeviceSummary {
        AudioDeviceSummary { id: name.into(), name: name.into(), is_default: false }
    }

    #[test]
    fn standard_vb_cable_is_the_only_admitted_pair() {
        let standard = select_route_pair(
            &[device("CABLE Input (VB-Audio Virtual Cable)")],
            &[device("CABLE Output (VB-Audio Virtual Cable)")],
        );
        assert!(standard.blocker.is_empty());
        assert_eq!(standard.selected_input_device.as_deref(), Some("CABLE Output (VB-Audio Virtual Cable)"));
        assert!(matched_pair_identity("TranslateIT Microphone Feed", "TranslateIT Meeting Microphone").is_none());

        for (playback, recording) in [
            ("Voicemeeter Input (VB-Audio Voicemeeter VAIO)", "Voicemeeter Output (VB-Audio Voicemeeter VAIO)"),
            ("CABLE-A Input (VB-Audio Cable A)", "CABLE-A Output (VB-Audio Cable A)"),
            ("Virtual Cable Input", "Virtual Cable Output"),
        ] {
            let unsupported = select_route_pair(&[device(playback)], &[device(recording)]);
            assert_eq!(unsupported.blocker, "virtual_mic:matched_route_pair_missing");
            assert!(unsupported.route_pair_id.is_none());
        }
    }

    #[test]
    fn prototype_names_can_never_activate_the_native_route() {
        for (playback, recording) in [
            ("TranslateIT Microphone Feed", "TranslateIT Meeting Microphone"),
            ("TranslateIT Microphone Feed", "Other Microphone"),
            ("Other Speaker", "TranslateIT Meeting Microphone"),
        ] {
            let unsupported = select_route_pair(&[device(playback)], &[device(recording)]);
            assert_eq!(unsupported.blocker, "virtual_mic:unaccepted_native_candidate");
            assert!(unsupported.route_pair_id.is_none());
        }
    }

    #[test]
    fn presence_of_unaccepted_native_device_does_not_hijack_legacy_pair() {
        let selected = select_route_pair(
            &[device("CABLE Input (VB-Audio Virtual Cable)"), device("TranslateIT Microphone Feed")],
            &[device("CABLE Output (VB-Audio Virtual Cable)"), device("TranslateIT Meeting Microphone")],
        );
        assert!(selected.blocker.is_empty());
        assert_eq!(selected.selected_output_device.as_deref(), Some("CABLE Input (VB-Audio Virtual Cable)"));
        assert_eq!(selected.selected_input_device.as_deref(), Some("CABLE Output (VB-Audio Virtual Cable)"));

        let incomplete = select_route_pair(
            &[device("TranslateIT Microphone Feed"), device("CABLE Input (VB-Audio Virtual Cable)")],
            &[device("CABLE Output (VB-Audio Virtual Cable)")],
        );
        assert!(incomplete.blocker.is_empty());
        assert_eq!(incomplete.selected_input_device.as_deref(), Some("CABLE Output (VB-Audio Virtual Cable)"));
    }

    #[test]
    fn duplicated_standard_name_fails_closed() {
        let named = select_route_pair(
            &[device("CABLE Input (VB-Audio Virtual Cable)")],
            &[device("CABLE Output (VB-Audio Virtual Cable)")],
        );
        let inventory = AudioDeviceListReport {
            ok: true,
            input_devices: vec![device("CABLE Output (VB-Audio Virtual Cable)")],
            output_devices: vec![device("CABLE Input (VB-Audio Virtual Cable)")],
            ambiguous_input_names: vec!["CABLE Output (VB-Audio Virtual Cable)".into()],
            ambiguous_output_names: vec![],
            blocker: String::new(),
            note: String::new(),
        };
        assert_eq!(
            reject_ambiguous_pair(named, &inventory).blocker,
            "virtual_mic:matched_route_pair_ambiguous"
        );
    }
}
