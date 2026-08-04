use std::time::{SystemTime, UNIX_EPOCH};

use any2api_domain::{LogPageCursor, LogPagePosition};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;

const DEFAULT_PAGE_SIZE: u32 = 20;
const MAX_PAGE_SIZE: u32 = 100;
const LOG_WINDOW_MS: u64 = 3 * 24 * 60 * 60 * 1_000;
const MAX_CURSOR_CHARS: usize = 1_024;
const MAX_SORT_KEY_BYTES: usize = 256;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LogListQuery {
    cursor: Option<String>,
    page_size: Option<u32>,
}

pub(crate) struct LogPageRequest {
    pub(crate) cursor: Option<LogPageCursor>,
    pub(crate) page_size: u32,
    pub(crate) since_ms: u64,
}

#[derive(Clone, Copy)]
pub(crate) enum LogCursorKind {
    Request,
    System,
}

impl LogCursorKind {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Request => "r1",
            Self::System => "s1",
        }
    }

    pub(crate) fn encode(self, cursor: &LogPageCursor) -> String {
        let anchor = cursor.anchor();
        let anchor_id = URL_SAFE_NO_PAD.encode(anchor.request_id().as_bytes());
        match cursor.before() {
            Some(before) => format!(
                "{}.{anchor_started}.{anchor_id}.{before_started}.{before_id}",
                self.prefix(),
                anchor_started = anchor.started_at_ms(),
                before_started = before.started_at_ms(),
                before_id = URL_SAFE_NO_PAD.encode(before.request_id().as_bytes()),
            ),
            None => format!(
                "{}.{anchor_started}.{anchor_id}",
                self.prefix(),
                anchor_started = anchor.started_at_ms(),
            ),
        }
    }

    fn decode(self, value: &str) -> Option<LogPageCursor> {
        if value.len() > MAX_CURSOR_CHARS {
            return None;
        }
        let parts = value.split('.').collect::<Vec<_>>();
        match parts.as_slice() {
            [prefix, anchor_started, anchor_id] if *prefix == self.prefix() => Some(
                LogPageCursor::first(decode_position(anchor_started, anchor_id)?),
            ),
            [prefix, anchor_started, anchor_id, before_started, before_id]
                if *prefix == self.prefix() =>
            {
                LogPageCursor::next(
                    decode_position(anchor_started, anchor_id)?,
                    decode_position(before_started, before_id)?,
                )
            }
            _ => None,
        }
    }
}

impl LogListQuery {
    pub(crate) fn validate(self, kind: LogCursorKind) -> Option<LogPageRequest> {
        let page_size = self.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        if !(1..=MAX_PAGE_SIZE).contains(&page_size) {
            return None;
        }
        let cursor = match self.cursor {
            Some(value) => Some(kind.decode(&value)?),
            None => None,
        };
        Some(LogPageRequest {
            cursor,
            page_size,
            since_ms: unix_time_ms().saturating_sub(LOG_WINDOW_MS),
        })
    }
}

fn decode_position(started_at_ms: &str, request_id: &str) -> Option<LogPagePosition> {
    let started_at_ms = started_at_ms.parse::<u64>().ok()?;
    i64::try_from(started_at_ms).ok()?;
    let request_id = URL_SAFE_NO_PAD.decode(request_id).ok()?;
    if request_id.is_empty() || request_id.len() > MAX_SORT_KEY_BYTES {
        return None;
    }
    Some(LogPagePosition::new(
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
    fn defaults_and_validates_log_pages() {
        let page = LogListQuery {
            cursor: None,
            page_size: None,
        }
        .validate(LogCursorKind::Request)
        .expect("default page");
        assert!(page.cursor.is_none());
        assert_eq!(page.page_size, 20);

        let invalid_size = LogListQuery {
            cursor: None,
            page_size: Some(0),
        };
        assert!(invalid_size.validate(LogCursorKind::Request).is_none());
    }

    #[test]
    fn log_cursors_round_trip_and_are_type_isolated() {
        let cursor = LogPageCursor::next(
            LogPagePosition::new(300, "anchor.raw".into()),
            LogPagePosition::new(200, "before/raw".into()),
        )
        .expect("ordered cursor");
        let encoded = LogCursorKind::Request.encode(&cursor);
        let query = LogListQuery {
            cursor: Some(encoded.clone()),
            page_size: Some(50),
        }
        .validate(LogCursorKind::Request)
        .expect("request cursor");
        assert_eq!(query.cursor, Some(cursor));
        assert_eq!(query.page_size, 50);

        assert!(
            LogListQuery {
                cursor: Some(encoded),
                page_size: Some(50),
            }
            .validate(LogCursorKind::System)
            .is_none()
        );

        assert!(
            LogListQuery {
                cursor: Some(format!("r1.{}.YQ", u64::MAX)),
                page_size: Some(50),
            }
            .validate(LogCursorKind::Request)
            .is_none()
        );
    }
}
