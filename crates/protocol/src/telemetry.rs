use any2api_domain::{MAX_TOKEN_COUNT, TokenUsage};
use serde_json::Value;

pub(crate) fn token_usage(
    usage: Option<&Value>,
    input_path: &[&str],
    output_path: &[&str],
    cache_read_path: &[&str],
    cache_write_path: &[&str],
) -> TokenUsage {
    TokenUsage::new(
        token_at(usage, input_path),
        token_at(usage, output_path),
        token_at(usage, cache_read_path),
        token_at(usage, cache_write_path),
    )
}

pub(crate) fn event_type<'a>(event_name: Option<&'a str>, value: &'a Value) -> Option<&'a str> {
    value.get("type").and_then(Value::as_str).or(event_name)
}

pub(crate) fn non_empty_string(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty())
}

fn token_at(mut current: Option<&Value>, path: &[&str]) -> Option<u64> {
    for segment in path {
        current = current?.get(*segment);
    }
    current?.as_u64().filter(|value| *value <= MAX_TOKEN_COUNT)
}
