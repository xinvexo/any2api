use std::{borrow::Cow, collections::BTreeMap};

use any2api_domain::ProtocolOperation;
use bytes::Bytes;
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::{ProviderError, api::ProviderRequestContext};

const REQUIRED_INCLUDE: &[u8] = br#"["reasoning.encrypted_content"]"#;
const REMOVED_FIELDS: &[&str] = &[
    "context_management",
    "max_completion_tokens",
    "max_output_tokens",
    "temperature",
    "top_p",
    "truncation",
    "user",
];

pub(crate) fn prepare(
    context: ProviderRequestContext<'_>,
    body: Bytes,
) -> Result<Bytes, ProviderError> {
    if !context.oauth || context.upstream_operation != ProtocolOperation::Responses {
        return Ok(body);
    }
    normalize_responses(body)
}

fn normalize_responses(body: Bytes) -> Result<Bytes, ProviderError> {
    let fields = raw_object(&body)?;
    let rewritten_input = fields
        .get("input")
        .map(|input| rewrite_input(input))
        .transpose()?
        .flatten();
    let store_is_false = fields
        .get("store")
        .is_some_and(|value| serde_json::from_str::<bool>(value.get()).is_ok_and(|value| !value));
    let include_is_required = fields.get("include").is_some_and(|value| {
        serde_json::from_str::<Vec<Cow<'_, str>>>(value.get())
            .is_ok_and(|values| values.len() == 1 && values[0] == "reasoning.encrypted_content")
    });
    let removes_field = fields
        .iter()
        .any(|(name, value)| should_remove(name, value));
    let parallel_needs_default = fields
        .get("parallel_tool_calls")
        .is_none_or(|value| serde_json::from_str::<bool>(value.get()).is_err());
    if store_is_false
        && include_is_required
        && !parallel_needs_default
        && !removes_field
        && rewritten_input.is_none()
    {
        return Ok(body);
    }

    let mut output = Vec::with_capacity(body.len().saturating_add(96));
    output.push(b'{');
    let mut first = true;
    for (name, value) in &fields {
        if should_remove(name, value) {
            continue;
        }
        let replacement = match name.as_str() {
            "store" => Some(b"false".as_slice()),
            "include" => Some(REQUIRED_INCLUDE),
            "input" => rewritten_input.as_deref(),
            "parallel_tool_calls" if parallel_needs_default => Some(b"true".as_slice()),
            _ => None,
        };
        write_field(
            &mut output,
            &mut first,
            name,
            replacement.unwrap_or_else(|| value.get().as_bytes()),
        )?;
    }
    if !fields.contains_key("store") {
        write_field(&mut output, &mut first, "store", b"false")?;
    }
    if !fields.contains_key("include") {
        write_field(&mut output, &mut first, "include", REQUIRED_INCLUDE)?;
    }
    if !fields.contains_key("parallel_tool_calls") {
        write_field(&mut output, &mut first, "parallel_tool_calls", b"true")?;
    }
    output.push(b'}');
    Ok(Bytes::from(output))
}

fn raw_object(body: &[u8]) -> Result<BTreeMap<String, &RawValue>, ProviderError> {
    serde_json::from_slice(body).map_err(|_| invalid_request())
}

fn should_remove(name: &str, value: &RawValue) -> bool {
    REMOVED_FIELDS.contains(&name)
        || (name == "service_tier"
            && serde_json::from_str::<Cow<'_, str>>(value.get())
                .map(|value| value != "priority")
                .unwrap_or(true))
}

fn rewrite_input(input: &RawValue) -> Result<Option<Vec<u8>>, ProviderError> {
    match input.get().as_bytes().first().copied() {
        Some(b'"') => Ok(Some(wrap_string_input(input))),
        Some(b'[') if contains_system_role_candidate(input.get().as_bytes()) => {
            rewrite_system_roles(input)
        }
        _ => Ok(None),
    }
}

fn wrap_string_input(input: &RawValue) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.get().len().saturating_add(80));
    output.extend_from_slice(
        br#"[{"type":"message","role":"user","content":[{"type":"input_text","text":"#,
    );
    output.extend_from_slice(input.get().as_bytes());
    output.extend_from_slice(br#"}]}]"#);
    output
}

fn contains_system_role_candidate(input: &[u8]) -> bool {
    input
        .windows(b"\"role\"".len())
        .enumerate()
        .any(|(index, window)| {
            if window != b"\"role\"" {
                return false;
            }
            let mut tail = &input[index + window.len()..];
            tail = trim_ascii_start(tail);
            let Some(after_colon) = tail.strip_prefix(b":") else {
                return false;
            };
            trim_ascii_start(after_colon).starts_with(b"\"system\"")
        })
}

fn trim_ascii_start(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    value
}

fn rewrite_system_roles(input: &RawValue) -> Result<Option<Vec<u8>>, ProviderError> {
    let items =
        serde_json::from_str::<Vec<&RawValue>>(input.get()).map_err(|_| invalid_request())?;
    let system_roles = items
        .iter()
        .map(|item| is_system_role(item))
        .collect::<Vec<_>>();
    if !system_roles.iter().any(|is_system| *is_system) {
        return Ok(None);
    }

    let mut output = Vec::with_capacity(input.get().len());
    output.push(b'[');
    for (index, (item, is_system)) in items.iter().zip(system_roles).enumerate() {
        if index != 0 {
            output.push(b',');
        }
        if is_system {
            write_developer_item(&mut output, item)?;
        } else {
            output.extend_from_slice(item.get().as_bytes());
        }
    }
    output.push(b']');
    Ok(Some(output))
}

#[derive(Deserialize)]
struct ItemRole<'a> {
    #[serde(borrow)]
    role: Option<Cow<'a, str>>,
}

fn is_system_role(item: &RawValue) -> bool {
    serde_json::from_str::<ItemRole<'_>>(item.get())
        .ok()
        .and_then(|item| item.role)
        .is_some_and(|role| role == "system")
}

fn write_developer_item(output: &mut Vec<u8>, item: &RawValue) -> Result<(), ProviderError> {
    let fields = raw_object(item.get().as_bytes())?;
    output.push(b'{');
    let mut first = true;
    for (name, value) in fields {
        let replacement = (name == "role").then_some(b"\"developer\"".as_slice());
        write_field(
            output,
            &mut first,
            &name,
            replacement.unwrap_or_else(|| value.get().as_bytes()),
        )?;
    }
    output.push(b'}');
    Ok(())
}

fn write_field(
    output: &mut Vec<u8>,
    first: &mut bool,
    name: &str,
    value: &[u8],
) -> Result<(), ProviderError> {
    if !*first {
        output.push(b',');
    }
    *first = false;
    serde_json::to_writer(&mut *output, name).map_err(|_| invalid_request())?;
    output.push(b':');
    output.extend_from_slice(value);
    Ok(())
}

fn invalid_request() -> ProviderError {
    ProviderError::InvalidRequest("Codex OAuth Responses request could not be normalized".into())
}

#[cfg(test)]
mod tests;
