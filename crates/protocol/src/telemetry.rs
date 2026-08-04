use std::borrow::Cow;

use any2api_domain::{MAX_TOKEN_COUNT, TokenUsage};
use serde_json::{Value, value::RawValue};

use crate::raw_json::{json_string, object_field_raw};

/// Token usage from a fully parsed response body.
pub(crate) fn token_usage(
    usage: Option<&Value>,
    input_path: &[&str],
    output_path: &[&str],
    cache_read_path: &[&str],
) -> TokenUsage {
    TokenUsage::new(
        token_at(usage, input_path),
        token_at(usage, output_path),
        token_at(usage, cache_read_path),
    )
}

/// Token usage probed from raw event bytes without building a value tree.
pub(crate) fn raw_token_usage(
    usage: Option<&RawValue>,
    input_path: &[&str],
    output_path: &[&str],
    cache_read_path: &[&str],
) -> TokenUsage {
    TokenUsage::new(
        raw_token_at(usage, input_path),
        raw_token_at(usage, output_path),
        raw_token_at(usage, cache_read_path),
    )
}

/// The event kind: an in-band `type` field wins over the SSE `event:` name.
pub(crate) fn raw_event_type<'a>(
    event_name: Option<&'a str>,
    kind: Option<&'a RawValue>,
) -> Option<Cow<'a, str>> {
    kind.and_then(json_string).or(event_name.map(Cow::Borrowed))
}

pub(crate) fn raw_non_empty_string(value: Option<&RawValue>) -> bool {
    value
        .and_then(json_string)
        .is_some_and(|value| !value.is_empty())
}

fn token_at(mut current: Option<&Value>, path: &[&str]) -> Option<u64> {
    for segment in path {
        current = current?.get(*segment);
    }
    current?.as_u64().filter(|value| *value <= MAX_TOKEN_COUNT)
}

fn raw_token_at(mut current: Option<&RawValue>, path: &[&str]) -> Option<u64> {
    for segment in path {
        current = object_field_raw(current?.get().as_bytes(), segment);
    }
    serde_json::from_str::<u64>(current?.get())
        .ok()
        .filter(|value| *value <= MAX_TOKEN_COUNT)
}
