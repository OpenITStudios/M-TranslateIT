//! Source-derived Codex App Server diagnostic compatibility gate.
//!
//! The upstream InitializeResponse contains userAgent, codexHome,
//! platformFamily and platformOs, NOT serverVersion. Its user-agent
//! starts with originator/CARGO_PKG_VERSION. Do not treat that value or
//! a successful account read as a realtime/Voice capability or billing proof.
//! This module does not spawn a second CLI process or expose codexHome.

use serde_json::Value;

const MAX_USER_AGENT_BYTES: usize = 1024;
const MAX_HOME_BYTES: usize = 4096;
const MAX_VERSION_BYTES: usize = 64;
const MAX_ORIGINATOR_BYTES: usize = 96;

pub(super) struct DiagnosticCompatibility {
    pub cli_version: String,
}

fn parse_build_version(user_agent: &str) -> Result<String, &'static str> {
    if user_agent.is_empty()
        || user_agent.len() > MAX_USER_AGENT_BYTES
        || user_agent.chars().any(char::is_control)
    {
        return Err("codex_compatibility:user_agent_invalid");
    }
    let token = user_agent.split_ascii_whitespace().next()
        .ok_or("codex_compatibility:user_agent_invalid")?;
    let (originator, version) = token.split_once('/')
        .ok_or("codex_compatibility:cli_version_missing")?;
    if originator.is_empty()
        || originator.len() > MAX_ORIGINATOR_BYTES
        || !originator.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')
        })
        || version.is_empty()
        || version.len() > MAX_VERSION_BYTES
        || !version.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+')
        })
    {
        return Err("codex_compatibility:cli_version_invalid");
    }
    let core = version.split(['-', '+']).next()
        .ok_or("codex_compatibility:cli_version_invalid")?;
    let mut numeric = core.split('.');
    let segments = [
        numeric.next(),
        numeric.next(),
        numeric.next(),
    ];
    if numeric.next().is_some() {
        return Err("codex_compatibility:cli_version_invalid");
    }
    let mut numbers = [0u32; 3];
    for (index, item) in segments.into_iter().enumerate() {
        let part = item.ok_or("codex_compatibility:cli_version_invalid")?;
        if part.is_empty()
            || part.len() > 9
            || (part.len() > 1 && part.starts_with('0'))
            || !part.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("codex_compatibility:cli_version_invalid");
        }
        numbers[index] = part.parse::<u32>()
            .map_err(|_| "codex_compatibility:cli_version_invalid")?;
    }
    // Upstream workspace source carries 0.0.0 as a development placeholder.
    // It is not an attested released-build identifier.
    if numbers == [0, 0, 0] {
        return Err("codex_compatibility:cli_version_placeholder");
    }
    if version.ends_with('-')
        || version.ends_with('+')
        || version.contains("..")
        || version.contains("--")
        || version.contains("++")
        || version.contains("-+")
        || version.contains("+-")
    {
        return Err("codex_compatibility:cli_version_invalid");
    }
    Ok(version.to_string())
}

/// Verify only the account-diagnostic subset of the upstream v1 initialize
/// response. Version syntax and this response shape do NOT certify the
/// experimental realtime APIs; P1 must keep those methods unadmitted.
pub(super) fn verify_initialize(
    result: &Value,
) -> Result<DiagnosticCompatibility, &'static str> {
    let obj = result.as_object()
        .ok_or("codex_compatibility:initialize_shape_invalid")?;
    if obj.get("platformFamily").and_then(Value::as_str) != Some("windows")
        || obj.get("platformOs").and_then(Value::as_str) != Some("windows")
    {
        return Err("codex_compatibility:server_platform_mismatch");
    }
    let codex_home = obj.get("codexHome").and_then(Value::as_str)
        .ok_or("codex_compatibility:codex_home_unverifiable")?;
    if codex_home.is_empty()
        || codex_home.len() > MAX_HOME_BYTES
        || codex_home.chars().any(char::is_control)
        || (cfg!(windows) && !std::path::Path::new(codex_home).is_absolute())
    {
        return Err("codex_compatibility:codex_home_unverifiable");
    }
    let version = parse_build_version(
        obj.get("userAgent").and_then(Value::as_str)
            .ok_or("codex_compatibility:cli_version_missing")?,
    )?;
    Ok(DiagnosticCompatibility { cli_version: version })
}

/// The explicitGatewayOauth capability is not acknowledged by initialize.
/// The later, read-only gatewayOAuth status probe must exist and describe
/// the selected builtin OpenAI provider. External gateway credentials and
/// ambiguous provider state are not allowed to bootstrap diagnostics.
pub(super) fn verify_gateway_oauth_read(
    result: &Value,
) -> Result<(), &'static str> {
    let obj = result.as_object()
        .ok_or("codex_compatibility:gateway_oauth_unverified")?;
    if obj.get("providerId").and_then(Value::as_str) != Some("openai")
        || obj.get("required").and_then(Value::as_bool) != Some(false)
        || !obj.get("status").is_some_and(Value::is_null)
        || !obj.get("error").is_some_and(Value::is_null)
    {
        return Err("codex_compatibility:gateway_provider_not_admitted");
    }
    let name = obj.get("providerName").and_then(Value::as_str)
        .ok_or("codex_compatibility:gateway_oauth_unverified")?;
    if name.trim().is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err("codex_compatibility:gateway_oauth_unverified");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn windows_initialize(user_agent: &str) -> Value {
        json!({
            "userAgent": user_agent,
            "codexHome": r"C:\Users\Demo\.codex",
            "platformFamily": "windows",
            "platformOs": "windows",
        })
    }
    fn builtin_provider() -> Value {
        json!({
            "providerId": "openai",
            "providerName": "OpenAI",
            "required": false,
            "status": null,
            "error": null,
        })
    }

    #[test]
    fn source_derived_initialize_returns_only_bounded_cli_version() {
        let compatible = verify_initialize(&windows_initialize(
            "translateit/0.110.0 (Windows 11; x64) client-terminal",
        )).unwrap();
        assert_eq!(compatible.cli_version, "0.110.0");
        let compatible = verify_initialize(&windows_initialize(
            "codex_cli_rs/1.2.3-rc.1+build.5 (Windows 11; x64)",
        )).unwrap();
        assert_eq!(compatible.cli_version, "1.2.3-rc.1+build.5");
    }

    #[test]
    fn invalid_or_placeholder_version_fails_closed_without_guessing_release_range() {
        for agent in [
            "",
            "translateit",
            "translateit/0.0.0",
            "translateit/unknown",
            "translateit/01.2.3",
            "translateit/1.2",
            "translateit/1.2.3.4",
            "translateit/1.2.3-",
            "translateit/1.2.3--invalid",
            "translateit/1.2.3/garbage",
        ] {
            assert!(verify_initialize(&windows_initialize(agent)).is_err(), "{agent}");
        }
    }

    #[test]
    fn platform_and_codex_home_must_be_source_derived_but_not_exposed() {
        let mut wrong_os = windows_initialize("translateit/0.110.0");
        wrong_os["platformOs"] = json!("linux");
        assert_eq!(
            verify_initialize(&wrong_os).err(),
            Some("codex_compatibility:server_platform_mismatch"),
        );
        let mut missing = windows_initialize("translateit/0.110.0");
        missing.as_object_mut().unwrap().remove("codexHome");
        assert_eq!(
            verify_initialize(&missing).err(),
            Some("codex_compatibility:codex_home_unverifiable"),
        );
        let mut private = windows_initialize("translateit/0.110.0");
        private["codexHome"] = json!("PRIVATE\nHOME");
        assert!(verify_initialize(&private).is_err());
    }

    #[test]
    fn read_only_gateway_oauth_probe_denies_external_paid_provider() {
        verify_gateway_oauth_read(&builtin_provider()).unwrap();
        for invalid in [
            json!({"providerId":"custom","providerName":"Custom","required":false,"status":null,"error":null}),
            json!({"providerId":"openai","providerName":"OpenAI","required":true,"status":"succeeded","error":null}),
            json!({"providerId":"openai","providerName":"OpenAI","required":false,"status":null}),
            json!({}),
        ] {
            assert!(verify_gateway_oauth_read(&invalid).is_err());
        }
    }
}
