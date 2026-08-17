use std::time::{SystemTime, UNIX_EPOCH};

use any2api_domain::{LogCursor, LogCursorPosition};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

pub(crate) const LOG_BATCH_SIZE: u32 = 100;
const LOG_WINDOW_MS: u64 = 3 * 24 * 60 * 60 * 1_000;
const MAX_CURSOR_CHARS: usize = 1_024;
const MAX_SORT_KEY_BYTES: usize = 256;

pub(crate) struct LogBatchRequest {
    pub(crate) cursor: Option<LogCursor>,
    pub(crate) since_ms: u64,
}

#[derive(Clone, Copy)]
pub(crate) enum LogCursorScope<'a> {
    Request(&'a str),
    System(bool),
}

impl LogCursorScope<'_> {
    fn prefix(self) -> String {
        match self {
            Self::Request(fingerprint) => format!("r4.{fingerprint}"),
            Self::System(show_admin_operations) => {
                format!("s5.{}", u8::from(show_admin_operations))
            }
        }
    }

    pub(crate) fn encode(self, cursor: &LogCursor) -> String {
        let anchor = cursor.anchor();
        let before = cursor
            .before()
            .expect("a continuation cursor always has an exclusive boundary");
        format!(
            "{prefix}.{anchor_started}.{anchor_id}.{before_started}.{before_id}",
            prefix = self.prefix(),
            anchor_started = anchor.started_at_ms(),
            anchor_id = URL_SAFE_NO_PAD.encode(anchor.request_id().as_bytes()),
            before_started = before.started_at_ms(),
            before_id = URL_SAFE_NO_PAD.encode(before.request_id().as_bytes()),
        )
    }

    fn decode(self, value: &str) -> Option<LogCursor> {
        if value.len() > MAX_CURSOR_CHARS {
            return None;
        }
        let prefix = self.prefix();
        let positions = value.strip_prefix(&prefix)?.strip_prefix('.')?;
        decode_cursor_positions(positions)
    }
}

pub(crate) fn validate_system_log_batch(
    cursor: Option<String>,
    show_admin_operations: bool,
) -> Option<LogBatchRequest> {
    validate_log_batch(cursor, LogCursorScope::System(show_admin_operations))
}

pub(crate) fn validate_request_log_batch(
    cursor: Option<String>,
    filter_fingerprint: &str,
) -> Option<LogBatchRequest> {
    validate_log_batch(cursor, LogCursorScope::Request(filter_fingerprint))
}

fn validate_log_batch(
    cursor: Option<String>,
    scope: LogCursorScope<'_>,
) -> Option<LogBatchRequest> {
    let cursor = match cursor {
        Some(value) => Some(scope.decode(&value)?),
        None => None,
    };
    Some(LogBatchRequest {
        cursor,
        since_ms: unix_time_ms().saturating_sub(LOG_WINDOW_MS),
    })
}

fn decode_cursor_positions(value: &str) -> Option<LogCursor> {
    let parts = value.split('.').collect::<Vec<_>>();
    let [anchor_started, anchor_id, before_started, before_id] = parts.as_slice() else {
        return None;
    };
    LogCursor::next(
        decode_position(anchor_started, anchor_id)?,
        decode_position(before_started, before_id)?,
    )
}

fn decode_position(started_at_ms: &str, request_id: &str) -> Option<LogCursorPosition> {
    let started_at_ms = started_at_ms.parse::<u64>().ok()?;
    i64::try_from(started_at_ms).ok()?;
    let request_id = URL_SAFE_NO_PAD.decode(request_id).ok()?;
    if request_id.is_empty() || request_id.len() > MAX_SORT_KEY_BYTES {
        return None;
    }
    Some(LogCursorPosition::new(
        started_at_ms,
        String::from_utf8(request_id).ok()?,
    ))
}

fn unix_time_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_a_fresh_fixed_batch() {
        let batch = validate_system_log_batch(None, true).expect("default batch");
        assert!(batch.cursor.is_none());
    }

    #[test]
    fn continuation_cursors_round_trip_and_are_scope_isolated() {
        let cursor = LogCursor::next(
            LogCursorPosition::new(300, "anchor.raw".into()),
            LogCursorPosition::new(200, "before/raw".into()),
        )
        .expect("ordered cursor");
        let encoded = LogCursorScope::Request("filters-a").encode(&cursor);
        let query =
            validate_request_log_batch(Some(encoded.clone()), "filters-a").expect("request cursor");
        assert_eq!(query.cursor, Some(cursor.clone()));

        assert!(validate_request_log_batch(Some(encoded.clone()), "filters-b").is_none());
        assert!(validate_system_log_batch(Some(encoded), true).is_none());

        let system_cursor = LogCursorScope::System(true).encode(&cursor);
        assert!(validate_system_log_batch(Some(system_cursor.clone()), true).is_some());
        assert!(validate_system_log_batch(Some(system_cursor), false).is_none());

        assert!(
            validate_request_log_batch(
                Some(format!("r4.filters-a.{}.YQ.1.Yg", u64::MAX)),
                "filters-a",
            )
            .is_none()
        );
    }
}
