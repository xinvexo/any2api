use sha2::{Digest, Sha256};

use crate::ProtocolError;

use super::{ChatToolKind, ToolIdentity};

pub(super) fn project_name(
    identity: &ToolIdentity,
    chat_kind: ChatToolKind,
    max_chars: usize,
    collides: impl Fn(&str) -> bool,
) -> Result<String, ProtocolError> {
    if max_chars < 17 {
        return Err(ProtocolError::InvalidPayload(
            "Chat target tool name limit is too small for reversible projection".into(),
        ));
    }
    let canonical = canonical(identity);
    let desired = match identity {
        ToolIdentity::Function { name } => sanitize(name),
        ToolIdentity::Custom { name } if chat_kind == ChatToolKind::Custom => sanitize(name),
        ToolIdentity::Custom { name } => format!("any2api_custom__{}", sanitize(name)),
        ToolIdentity::NamespaceFunction { namespace, name } => {
            format!("{}__{}", sanitize(namespace), sanitize(name))
        }
        ToolIdentity::NamespaceCustom { namespace, name } => {
            format!(
                "any2api_custom__{}__{}",
                sanitize(namespace),
                sanitize(name)
            )
        }
        ToolIdentity::ToolSearch => "any2api_tool_search".to_owned(),
    };
    let lossless = matches!(identity, ToolIdentity::Function { name } if name == &desired)
        || matches!(identity, ToolIdentity::Custom { name } if chat_kind == ChatToolKind::Custom && name == &desired);
    if lossless && desired.chars().count() <= max_chars && !collides(&desired) {
        return Ok(desired);
    }
    let suffix = digest_suffix(canonical.as_bytes());
    let prefix_chars = max_chars.saturating_sub(suffix.len() + 1);
    let mut prefix = desired.chars().take(prefix_chars).collect::<String>();
    if prefix.is_empty() {
        prefix.push_str("tool");
        prefix.truncate(prefix_chars);
    }
    let projected = format!("{prefix}_{suffix}");
    if collides(&projected) {
        return Err(ProtocolError::InvalidPayload(
            "tool names collide after deterministic projection".into(),
        ));
    }
    Ok(projected)
}

fn sanitize(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        output.push_str("tool");
    }
    output
}

fn canonical(identity: &ToolIdentity) -> String {
    match identity {
        ToolIdentity::Function { name } => format!("function\0{name}"),
        ToolIdentity::Custom { name } => format!("custom\0{name}"),
        ToolIdentity::NamespaceFunction { namespace, name } => {
            format!("namespace-function\0{namespace}\0{name}")
        }
        ToolIdentity::NamespaceCustom { namespace, name } => {
            format!("namespace-custom\0{namespace}\0{name}")
        }
        ToolIdentity::ToolSearch => "tool-search".to_owned(),
    }
}

fn digest_suffix(input: &[u8]) -> String {
    let digest = Sha256::digest(input);
    let mut output = String::with_capacity(12);
    for byte in digest.iter().take(6) {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
