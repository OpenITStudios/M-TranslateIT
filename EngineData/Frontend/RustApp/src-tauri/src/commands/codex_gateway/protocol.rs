//! Narrow, fail-closed JSONL contract for a locally owned Codex App Server.
//! No model turn, tool execution, audio, or paid provider is authorized here.

use serde_json::{json, Value};

pub(super) const MAX_FRAME_BYTES: usize = 256 * 1024;
pub(super) const MAX_REQUEST_BYTES: usize = 8 * 1024;

#[derive(Debug)]
pub(super) enum Inbound {
    Response { id: u64, result: Value },
    Notification { method: String, params: Value },
}

pub(super) fn parse_frame(frame: &[u8]) -> Result<Inbound, &'static str> {
    if frame.is_empty() || frame.len() > MAX_FRAME_BYTES {
        return Err("codex_gateway:frame_size_invalid");
    }
    let value: Value = serde_json::from_slice(frame)
        .map_err(|_| "codex_gateway:frame_json_invalid")?;
    let object = value.as_object().ok_or("codex_gateway:frame_shape_invalid")?;
    if let Some(id) = object.get("id") {
        let id = id.as_u64().ok_or("codex_gateway:response_id_invalid")?;
        if object.get("error").is_some_and(|error| !error.is_null()) {
            // Never surface upstream error text: it can contain private context.
            return Err("codex_gateway:rpc_rejected");
        }
        let result = object.get("result").ok_or("codex_gateway:result_missing")?;
        if !result.is_object() {
            return Err("codex_gateway:result_shape_invalid");
        }
        return Ok(Inbound::Response { id, result: result.clone() });
    }
    let method = object.get("method").and_then(Value::as_str)
        .ok_or("codex_gateway:notification_invalid")?;
    if method.is_empty()
        || method.len() > 128
        || !method.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-' | b'.'))
    {
        return Err("codex_gateway:notification_method_invalid");
    }
    let params = object.get("params").cloned().unwrap_or(Value::Null);
    Ok(Inbound::Notification { method: method.to_string(), params })
}

pub(super) fn request(id: u64, method: &str, params: Value) -> Result<Vec<u8>, &'static str> {
    // The caller owns the fixed allowlist. This function refuses arbitrary method names.
    if !matches!(method, "initialize" | "account/read" | "account/rateLimits/read")
        || !params.is_object()
    {
        return Err("codex_gateway:request_not_allowed");
    }
    let mut body = serde_json::to_vec(&json!({
        "id": id, "method": method, "params": params
    })).map_err(|_| "codex_gateway:request_encode_failed")?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err("codex_gateway:request_too_large");
    }
    body.push(b'\n');
    Ok(body)
}

pub(super) fn initialized_notification() -> &'static [u8] {
    b"{\"method\":\"initialized\"}\n"
}

pub(super) fn chatgpt_account(result: &Value) -> Result<(), &'static str> {
    let account = result.get("account").ok_or("codex_gateway:account_unverifiable")?;
    if account.is_null() {
        return Err("codex_gateway:chatgpt_login_required");
    }
    match account.get("type").and_then(Value::as_str) {
        Some("chatgpt") => Ok(()),
        Some(_) => Err("codex_gateway:non_chatgpt_provider"),
        None => Err("codex_gateway:account_unverifiable"),
    }
}

/// True: included usage explicitly permitted. False: quota explicitly blocked.
/// None: no current authoritative permission, never silently assumed allowed.
pub(super) fn ordinary_usage(result: &Value) -> Result<Option<bool>, &'static str> {
    match result.get("ordinaryUsageAllowed") {
        Some(Value::Bool(allowed)) => Ok(Some(*allowed)),
        Some(Value::Null) => Ok(None),
        Some(_) => Err("codex_gateway:usage_permission_invalid"),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_matching_responses_and_notifications_without_sensitive_error_text() {
        match parse_frame(br#"{"id":7,"result":{"account":{"type":"chatgpt"}}}"#).unwrap() {
            Inbound::Response { id, result } => {
                assert_eq!(id, 7);
                chatgpt_account(&result).unwrap();
            }
            _ => panic!("expected response"),
        }
        assert!(matches!(
            parse_frame(br#"{"method":"account/updated","params":{}}"#).unwrap(),
            Inbound::Notification { method, .. } if method == "account/updated"
        ));
        assert_eq!(
            parse_frame(br#"{"id":1,"error":{"message":"private email"}}"#).unwrap_err(),
            "codex_gateway:rpc_rejected"
        );
    }

    #[test]
    fn refuses_paid_providers_and_unverified_included_usage() {
        assert_eq!(chatgpt_account(&json!({"account":{"type":"apiKey"}})),
            Err("codex_gateway:non_chatgpt_provider"));
        assert_eq!(chatgpt_account(&json!({"account":null})),
            Err("codex_gateway:chatgpt_login_required"));
        assert_eq!(ordinary_usage(&json!({"ordinaryUsageAllowed":true})), Ok(Some(true)));
        assert_eq!(ordinary_usage(&json!({"ordinaryUsageAllowed":false})), Ok(Some(false)));
        assert_eq!(ordinary_usage(&json!({"ordinaryUsageAllowed":null})), Ok(None));
        assert_eq!(ordinary_usage(&json!({})), Ok(None));
    }

    #[test]
    fn rejects_unbounded_or_unexpected_protocol_inputs() {
        assert_eq!(parse_frame(&vec![b' '; MAX_FRAME_BYTES + 1]).unwrap_err(),
            "codex_gateway:frame_size_invalid");
        assert!(parse_frame(br#"{"id":"7","result":{}}"#).is_err());
        assert!(parse_frame(br#"{"id":7,"result":[]}"#).is_err());
        assert!(parse_frame(br#"{"method":"../bad\nmethod"}"#).is_err());
        assert!(request(1, "turn/start", json!({})).is_err());
        assert!(request(1, "initialize", json!([])).is_err());
        assert!(request(1, "initialize", json!({})).unwrap().ends_with(b"\n"));
    }

    #[test]
    fn notification_params_are_preserved_with_session_identity() {
        let frame = br#"{"method":"thread/realtime/item/completed","params":{"threadId":"t","item":{"id":"i","realtimeSessionId":"s","type":"transcriptSegment","role":"assistant","text":"Hello"}}}"#;
        match parse_frame(frame).unwrap() {
            Inbound::Notification { method, params } => {
                assert_eq!(method, "thread/realtime/item/completed");
                assert_eq!(params["threadId"], "t");
                assert_eq!(params["item"]["realtimeSessionId"], "s");
            }
            _ => panic!("expected notification"),
        }
    }
}
