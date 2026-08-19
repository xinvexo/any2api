use std::{borrow::Cow, collections::BTreeMap, io::Write};

use any2api_domain::ProtocolOperation;
use any2api_payload_buffer::PayloadBuffer;
use bytes::Bytes;
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::{ProviderError, api::ProviderRequestContext};

mod cache_key;

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
    if context.upstream_operation != ProtocolOperation::Responses {
        return Ok(body);
    }
    let body = cache_key::stabilize_memory_prompt_cache(context.upstream_model, body)?;
    if !context.oauth {
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

    let mut output = output_buffer(body.len().saturating_add(96))?;
    write_bytes(&mut output, b"{")?;
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
    write_bytes(&mut output, b"}")?;
    Ok(output.freeze().into_bytes())
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

fn rewrite_input(input: &RawValue) -> Result<Option<Bytes>, ProviderError> {
    match input.get().as_bytes().first().copied() {
        Some(b'"') => Ok(Some(wrap_string_input(input)?)),
        Some(b'[') if contains_system_role_candidate(input.get().as_bytes()) => {
            rewrite_system_roles(input)
        }
        _ => Ok(None),
    }
}

fn wrap_string_input(input: &RawValue) -> Result<Bytes, ProviderError> {
    let mut output = output_buffer(input.get().len().saturating_add(80))?;
    write_bytes(
        &mut output,
        br#"[{"type":"message","role":"user","content":[{"type":"input_text","text":"#,
    )?;
    write_bytes(&mut output, input.get().as_bytes())?;
    write_bytes(&mut output, br#"}]}]"#)?;
    Ok(output.freeze().into_bytes())
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

fn rewrite_system_roles(input: &RawValue) -> Result<Option<Bytes>, ProviderError> {
    let items =
        serde_json::from_str::<Vec<&RawValue>>(input.get()).map_err(|_| invalid_request())?;
    let system_roles = items
        .iter()
        .map(|item| is_system_role(item))
        .collect::<Vec<_>>();
    if !system_roles.iter().any(|is_system| *is_system) {
        return Ok(None);
    }

    let mut output = output_buffer(input.get().len())?;
    write_bytes(&mut output, b"[")?;
    for (index, (item, is_system)) in items.iter().zip(system_roles).enumerate() {
        if index != 0 {
            write_bytes(&mut output, b",")?;
        }
        if is_system {
            write_developer_item(&mut output, item)?;
        } else {
            write_bytes(&mut output, item.get().as_bytes())?;
        }
    }
    write_bytes(&mut output, b"]")?;
    Ok(Some(output.freeze().into_bytes()))
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

fn write_developer_item(output: &mut PayloadBuffer, item: &RawValue) -> Result<(), ProviderError> {
    let fields = raw_object(item.get().as_bytes())?;
    write_bytes(output, b"{")?;
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
    write_bytes(output, b"}")?;
    Ok(())
}

fn write_field(
    output: &mut PayloadBuffer,
    first: &mut bool,
    name: &str,
    value: &[u8],
) -> Result<(), ProviderError> {
    if !*first {
        write_bytes(output, b",")?;
    }
    *first = false;
    serde_json::to_writer(&mut *output, name).map_err(|_| processing_failed())?;
    write_bytes(output, b":")?;
    write_bytes(output, value)?;
    Ok(())
}

fn output_buffer(expected_len: usize) -> Result<PayloadBuffer, ProviderError> {
    PayloadBuffer::with_capacity_hint(Some(expected_len), usize::MAX)
        .map_err(|_| processing_failed())
}

fn write_bytes(output: &mut PayloadBuffer, value: &[u8]) -> Result<(), ProviderError> {
    output.write_all(value).map_err(|_| processing_failed())
}

fn invalid_request() -> ProviderError {
    ProviderError::InvalidRequest("Codex OAuth Responses request could not be normalized".into())
}

fn processing_failed() -> ProviderError {
    ProviderError::Internal("Codex OAuth Responses request normalization failed".into())
}

#[cfg(test)]
mod tests;
