use std::{borrow::Cow, collections::BTreeMap};

use any2api_payload_buffer::PayloadBuffer;
use bytes::Bytes;
use serde::Deserialize;
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

use crate::ProviderError;

use super::{output_buffer, raw_object, write_bytes, write_field};

const TURN_METADATA_KEY: &str = "x-codex-turn-metadata";
const MEMORY_REQUEST_KINDS: &[&str] = &["memory", "memory_consolidation"];
const GPT_56_MODEL_PREFIX: &str = "gpt-5.6-";
const VERSION_SALT: &[u8] = b"codex-memory/v1";
const FIELD_SEPARATOR: [u8; 1] = [0x00];
const ABSENT_EFFORT: [u8; 1] = [0x01];
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Codex scopes background memory extraction to the per-task session cache
/// key, so the globally fixed memory instructions never hit the upstream
/// prompt cache across tasks. Officially marked memory requests get a stable
/// key and an explicit breakpoint after their fixed instructions; anything
/// ambiguous passes through untouched, because misfiring on a normal turn
/// would merge every session onto one cache shard (ADR-0154).
pub(super) fn stabilize_memory_prompt_cache(
    upstream_model: &str,
    body: Bytes,
) -> Result<Bytes, ProviderError> {
    if !supports_explicit_memory_cache(upstream_model) || !contains_turn_metadata_marker(&body) {
        return Ok(body);
    }
    let Ok(fields) = raw_object(&body) else {
        return Ok(body);
    };
    let Some(instructions) = memory_instructions(&fields) else {
        return Ok(body);
    };
    let Some(input) = rewrite_memory_input(fields.get("input").copied(), &instructions)? else {
        return Ok(body);
    };
    let Some(prompt_cache_options) =
        rewrite_prompt_cache_options(fields.get("prompt_cache_options").copied())?
    else {
        return Ok(body);
    };
    let key = derive_key(upstream_model, effort(&fields).as_deref(), &instructions);
    rewrite_memory_prompt_cache(&fields, &key, &input, &prompt_cache_options, body.len())
}

fn supports_explicit_memory_cache(upstream_model: &str) -> bool {
    // Codex model aliases retain the `gpt-5.6-` family prefix, so capability
    // follows the actual upstream name rather than the public model alias.
    upstream_model == "gpt-5.6"
        || upstream_model
            .strip_prefix(GPT_56_MODEL_PREFIX)
            .is_some_and(|suffix| !suffix.is_empty())
}

fn contains_turn_metadata_marker(body: &[u8]) -> bool {
    body.windows(TURN_METADATA_KEY.len())
        .any(|window| window == TURN_METADATA_KEY.as_bytes())
}

#[derive(Deserialize)]
struct TurnMetadata<'a> {
    #[serde(borrow)]
    request_kind: Option<Cow<'a, str>>,
}

fn memory_instructions(fields: &BTreeMap<String, &RawValue>) -> Option<String> {
    let metadata = fields.get("client_metadata")?;
    let entries: BTreeMap<Cow<'_, str>, &RawValue> = serde_json::from_str(metadata.get()).ok()?;
    let turn_metadata: Cow<'_, str> =
        serde_json::from_str(entries.get(TURN_METADATA_KEY)?.get()).ok()?;
    let turn_metadata: TurnMetadata<'_> = serde_json::from_str(&turn_metadata).ok()?;
    let request_kind = turn_metadata.request_kind?;
    if !MEMORY_REQUEST_KINDS.contains(&request_kind.as_ref()) {
        return None;
    }
    let instructions: String = serde_json::from_str(fields.get("instructions")?.get()).ok()?;
    (!instructions.is_empty()).then_some(instructions)
}

#[derive(Deserialize)]
struct Reasoning {
    effort: Option<String>,
}

fn effort(fields: &BTreeMap<String, &RawValue>) -> Option<String> {
    let reasoning = fields.get("reasoning")?;
    serde_json::from_str::<Reasoning>(reasoning.get())
        .ok()?
        .effort
}

fn derive_key(upstream_model: &str, effort: Option<&str>, instructions: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(VERSION_SALT);
    hasher.update(FIELD_SEPARATOR);
    hasher.update(upstream_model.as_bytes());
    hasher.update(FIELD_SEPARATOR);
    match effort {
        Some(effort) => hasher.update(effort.as_bytes()),
        None => hasher.update(ABSENT_EFFORT),
    }
    hasher.update(FIELD_SEPARATOR);
    hasher.update(instructions.as_bytes());
    format_uuid(hasher.finalize().as_slice())
}

/// The wire shape matches the session UUIDs Codex sends on its own: the key
/// must not advertise the gateway to the upstream.
fn format_uuid(digest: &[u8]) -> String {
    let mut id = [0_u8; 16];
    id.copy_from_slice(&digest[..16]);
    id[6] = (id[6] & 0x0F) | 0x40;
    id[8] = (id[8] & 0x3F) | 0x80;
    let mut formatted = String::with_capacity(36);
    for (index, byte) in id.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            formatted.push('-');
        }
        formatted.push(HEX_DIGITS[usize::from(byte >> 4)] as char);
        formatted.push(HEX_DIGITS[usize::from(byte & 0x0F)] as char);
    }
    formatted
}

fn rewrite_memory_input(
    input: Option<&RawValue>,
    instructions: &str,
) -> Result<Option<Bytes>, ProviderError> {
    // The Responses API only permits cache breakpoints on supported input
    // blocks, not on the top-level `instructions` string. A leading developer
    // message preserves that instruction priority and leaves rollout input
    // strictly after the reusable boundary.
    let input_len = input.map_or(0, |value| value.get().len());
    let mut output = output_buffer(
        input_len
            .saturating_add(instructions.len())
            .saturating_add(160),
    )?;
    write_bytes(
        &mut output,
        br#"[{"type":"message","role":"developer","content":[{"type":"input_text","text":"#,
    )?;
    serde_json::to_writer(&mut output, instructions).map_err(|_| rewrite_failed())?;
    write_bytes(
        &mut output,
        br#","prompt_cache_breakpoint":{"mode":"explicit"}}]}"#,
    )?;

    match input {
        None => {}
        Some(value) if value.get().trim_start().starts_with('"') => {
            write_bytes(&mut output, b",")?;
            write_user_string_input(&mut output, value)?;
        }
        Some(value) if value.get().trim_start().starts_with('[') => {
            let Ok(items) = serde_json::from_str::<Vec<&RawValue>>(value.get()) else {
                return Ok(None);
            };
            for item in items {
                write_bytes(&mut output, b",")?;
                write_bytes(&mut output, item.get().as_bytes())?;
            }
        }
        Some(_) => return Ok(None),
    }
    write_bytes(&mut output, b"]")?;
    Ok(Some(output.freeze().into_bytes()))
}

fn write_user_string_input(
    output: &mut PayloadBuffer,
    input: &RawValue,
) -> Result<(), ProviderError> {
    write_bytes(
        output,
        br#"{"type":"message","role":"user","content":[{"type":"input_text","text":"#,
    )?;
    write_bytes(output, input.get().as_bytes())?;
    write_bytes(output, br#"}]}"#)
}

fn rewrite_prompt_cache_options(value: Option<&RawValue>) -> Result<Option<Bytes>, ProviderError> {
    let Some(value) = value else {
        return Ok(Some(Bytes::from_static(br#"{"mode":"explicit"}"#)));
    };
    if !value.get().trim_start().starts_with('{') {
        return Ok(None);
    }
    let Ok(fields) = serde_json::from_str::<BTreeMap<String, &RawValue>>(value.get()) else {
        return Ok(None);
    };
    let mut output = output_buffer(value.get().len().saturating_add(24))?;
    write_bytes(&mut output, b"{")?;
    let mut first = true;
    for (name, value) in &fields {
        let replacement = (name == "mode").then_some(br#""explicit""#.as_slice());
        write_field(
            &mut output,
            &mut first,
            name,
            replacement.unwrap_or_else(|| value.get().as_bytes()),
        )?;
    }
    if !fields.contains_key("mode") {
        write_field(&mut output, &mut first, "mode", br#""explicit""#)?;
    }
    write_bytes(&mut output, b"}")?;
    Ok(Some(output.freeze().into_bytes()))
}

fn rewrite_memory_prompt_cache(
    fields: &BTreeMap<String, &RawValue>,
    key: &str,
    input: &Bytes,
    prompt_cache_options: &Bytes,
    body_len: usize,
) -> Result<Bytes, ProviderError> {
    let encoded = serde_json::to_string(key).map_err(|_| rewrite_failed())?;
    let mut output = output_buffer(
        body_len
            .saturating_add(encoded.len())
            .saturating_add(input.len())
            .saturating_add(prompt_cache_options.len())
            .saturating_add(48),
    )?;
    write_bytes(&mut output, b"{")?;
    let mut first = true;
    let mut has_input = false;
    let mut has_prompt_cache_options = false;
    let mut has_prompt_cache_key = false;
    for (name, value) in fields {
        if name == "instructions" {
            continue;
        }
        let replacement = match name.as_str() {
            "input" => {
                has_input = true;
                Some(input.as_ref())
            }
            "prompt_cache_options" => {
                has_prompt_cache_options = true;
                Some(prompt_cache_options.as_ref())
            }
            "prompt_cache_key" => {
                has_prompt_cache_key = true;
                Some(encoded.as_bytes())
            }
            _ => None,
        };
        write_field(
            &mut output,
            &mut first,
            name,
            replacement.unwrap_or_else(|| value.get().as_bytes()),
        )?;
    }
    if !has_input {
        write_field(&mut output, &mut first, "input", input.as_ref())?;
    }
    if !has_prompt_cache_options {
        write_field(
            &mut output,
            &mut first,
            "prompt_cache_options",
            prompt_cache_options.as_ref(),
        )?;
    }
    if !has_prompt_cache_key {
        write_field(
            &mut output,
            &mut first,
            "prompt_cache_key",
            encoded.as_bytes(),
        )?;
    }
    write_bytes(&mut output, b"}")?;
    Ok(output.freeze().into_bytes())
}

fn rewrite_failed() -> ProviderError {
    ProviderError::Internal("Codex memory prompt cache rewrite failed".into())
}

#[cfg(test)]
mod tests;
