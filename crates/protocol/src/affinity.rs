use any2api_domain::ProtocolOperation;
use http::HeaderMap;
use serde_json::{Map, Value};

use crate::{ProtocolError, api::IngressAffinity};

const MAX_SESSION_ID_BYTES: usize = 4 * 1024;

pub(crate) fn extract(
    operation: ProtocolOperation,
    headers: &HeaderMap,
    object: &Map<String, Value>,
) -> Result<IngressAffinity, ProtocolError> {
    if operation == ProtocolOperation::Responses
        && let Some(previous) = previous_response_id(object)?
    {
        return Ok(IngressAffinity::Hard(previous));
    }
    if operation == ProtocolOperation::MessagesCountTokens {
        return Ok(IngressAffinity::None);
    }

    for (header, source) in [
        ("x-any2api-session", "any2api"),
        ("x-session-id", "session"),
        ("session-id", "codex"),
        ("session_id", "codex"),
    ] {
        if let Some(value) = header_value(headers, header)? {
            return Ok(IngressAffinity::Soft(namespaced(source, value)?));
        }
    }

    if let Some(value) = claude_session_id(object)? {
        return Ok(IngressAffinity::Soft(namespaced("claude", &value)?));
    }
    if let Some(value) = string_field(object, "conversation_id")? {
        return Ok(IngressAffinity::Soft(namespaced("conversation", value)?));
    }
    Ok(IngressAffinity::None)
}

fn previous_response_id(object: &Map<String, Value>) -> Result<Option<String>, ProtocolError> {
    let Some(value) = object.get("previous_response_id") else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let value = value.as_str().ok_or_else(|| {
        ProtocolError::InvalidPayload("previous_response_id must be a string or null".into())
    })?;
    Ok(normalized(value)?.map(str::to_owned))
}

fn claude_session_id(object: &Map<String, Value>) -> Result<Option<String>, ProtocolError> {
    let Some(user_id) = object
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("user_id"))
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };

    if let Ok(value) = serde_json::from_str::<Value>(user_id)
        && let Some(session_id) = value.get("session_id").and_then(Value::as_str)
    {
        return Ok(normalized(session_id)?.map(str::to_owned));
    }
    if let Some((_, session_id)) = user_id.rsplit_once("_session_") {
        return Ok(normalized(session_id)?.map(str::to_owned));
    }
    Ok(None)
}

fn string_field<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, ProtocolError> {
    let Some(value) = object.get(name) else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        return Ok(None);
    };
    normalized(value)
}

fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, ProtocolError> {
    let Some(value) = headers.get(name) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| ProtocolError::InvalidPayload("session header is not valid text".into()))?;
    normalized(value)
}

fn normalized(value: &str) -> Result<Option<&str>, ProtocolError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > MAX_SESSION_ID_BYTES {
        return Err(ProtocolError::InvalidPayload(
            "session identifier is too long".into(),
        ));
    }
    Ok(Some(value))
}

fn namespaced(source: &str, value: &str) -> Result<String, ProtocolError> {
    let value = normalized(value)?.expect("caller only passes non-empty values");
    Ok(format!("{source}:{value}"))
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProtocolOperation;
    use http::{HeaderMap, HeaderValue};
    use serde_json::json;

    use super::extract;
    use crate::api::IngressAffinity;

    #[test]
    fn explicit_sources_follow_the_architecture_priority() {
        let mut headers = HeaderMap::new();
        headers.insert("x-any2api-session", HeaderValue::from_static("explicit"));
        headers.insert("x-session-id", HeaderValue::from_static("fallback"));
        let body = json!({
            "previous_response_id": "resp_1",
            "metadata": {"user_id": "{\"session_id\":\"claude\"}"},
            "conversation_id": "conversation"
        });
        assert_eq!(
            extract(
                ProtocolOperation::Responses,
                &headers,
                body.as_object().expect("object"),
            )
            .expect("affinity"),
            IngressAffinity::Hard("resp_1".into())
        );

        let body = json!({"conversation_id": "conversation"});
        assert_eq!(
            extract(
                ProtocolOperation::Messages,
                &headers,
                body.as_object().expect("object"),
            )
            .expect("affinity"),
            IngressAffinity::Soft("any2api:explicit".into())
        );
    }

    #[test]
    fn claude_code_user_id_supports_json_and_legacy_forms() {
        for (user_id, expected) in [
            (
                r#"{"device_id":"d","session_id":"session-json"}"#,
                "claude:session-json",
            ),
            (
                "user_hash_account__session_session-legacy",
                "claude:session-legacy",
            ),
        ] {
            let body = json!({"metadata": {"user_id": user_id}});
            assert_eq!(
                extract(
                    ProtocolOperation::Messages,
                    &HeaderMap::new(),
                    body.as_object().expect("object"),
                )
                .expect("affinity"),
                IngressAffinity::Soft(expected.into())
            );
        }
    }

    #[test]
    fn count_tokens_and_content_without_explicit_ids_do_not_enable_affinity() {
        let body = json!({"messages": [{"role": "user", "content": "same prompt"}]});
        assert_eq!(
            extract(
                ProtocolOperation::Messages,
                &HeaderMap::new(),
                body.as_object().expect("object"),
            )
            .expect("affinity"),
            IngressAffinity::None
        );
        let mut headers = HeaderMap::new();
        headers.insert("x-session-id", HeaderValue::from_static("ignored"));
        assert_eq!(
            extract(
                ProtocolOperation::MessagesCountTokens,
                &headers,
                body.as_object().expect("object"),
            )
            .expect("affinity"),
            IngressAffinity::None
        );
    }
}
