use std::{borrow::Cow, collections::BTreeMap};

use bytes::Bytes;
use serde::Deserialize;
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

use crate::ProviderError;

use super::{output_buffer, raw_object, write_bytes, write_field};

const TURN_METADATA_KEY: &str = "x-codex-turn-metadata";
const MEMORY_REQUEST_KINDS: &[&str] = &["memory", "memory_consolidation"];
const VERSION_SALT: &[u8] = b"codex-memory/v1";
const FIELD_SEPARATOR: [u8; 1] = [0x00];
const ABSENT_EFFORT: [u8; 1] = [0x01];
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Codex scopes background memory extraction to the per-task session cache
/// key, so the globally fixed memory instructions never hit the upstream
/// prompt cache across tasks. Officially marked memory requests get a key
/// derived from the request content instead; anything ambiguous passes
/// through untouched, because misfiring on a normal turn would merge every
/// session onto one cache shard (ADR-0154).
pub(super) fn stabilize_memory_prompt_cache_key(
    upstream_model: &str,
    body: Bytes,
) -> Result<Bytes, ProviderError> {
    if !contains_turn_metadata_marker(&body) {
        return Ok(body);
    }
    let Ok(fields) = raw_object(&body) else {
        return Ok(body);
    };
    let Some(instructions) = memory_instructions(&fields) else {
        return Ok(body);
    };
    let key = derive_key(upstream_model, effort(&fields).as_deref(), &instructions);
    rewrite_prompt_cache_key(&fields, &key, body.len())
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

fn rewrite_prompt_cache_key(
    fields: &BTreeMap<String, &RawValue>,
    key: &str,
    body_len: usize,
) -> Result<Bytes, ProviderError> {
    let encoded = serde_json::to_string(key).map_err(|_| rewrite_failed())?;
    let mut output = output_buffer(body_len.saturating_add(encoded.len().saturating_add(24)))?;
    write_bytes(&mut output, b"{")?;
    let mut first = true;
    for (name, value) in fields {
        let replacement = (name == "prompt_cache_key").then_some(encoded.as_bytes());
        write_field(
            &mut output,
            &mut first,
            name,
            replacement.unwrap_or_else(|| value.get().as_bytes()),
        )?;
    }
    if !fields.contains_key("prompt_cache_key") {
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
    ProviderError::Internal("Codex memory prompt_cache_key rewrite failed".into())
}

#[cfg(test)]
mod tests;
