use std::fs::{self, OpenOptions};
use std::io::Write;

use serde_json::json;
use tempfile::tempdir;

use super::*;

fn event(message: &str, level: &str) -> String {
    format!(
        "{}\n",
        json!({
            "timestamp": "2026-09-08T02:30:00Z", "level": level,
            "target": "any2api_runtime::oauth::refresh::state", "message": message,
            "oauth_account_id": "account-a", "refresh_reason": "Rejected",
            "reauthorization_required": true,
        })
    )
}

#[test]
fn pages_retained_events_across_windows_rotation_and_concurrent_appends() {
    let directory = tempdir().unwrap();
    let older = directory.path().join("any2api-2026-09-07-000000.jsonl");
    fs::write(&older, event("oldest", "WARN")).unwrap();
    let active = directory.path().join("any2api-2026-09-08-000000.jsonl");
    let events: String = (0..130)
        .map(|index| event(&format!("{index}:{}", "x".repeat(3000)), "WARN"))
        .collect();
    fs::write(&active, events).unwrap();
    let first = read_page(directory.path(), &RuntimeLogQuery::default()).unwrap();
    assert_eq!(first.items.len(), 100);
    assert!(first.items[0].message.starts_with("129:"));
    assert!(first.items[99].message.starts_with("30:"));
    OpenOptions::new()
        .append(true)
        .open(&active)
        .unwrap()
        .write_all(event("new arrival", "WARN").as_bytes())
        .unwrap();
    let second = read_page(
        directory.path(),
        &RuntimeLogQuery {
            cursor: first.next_cursor,
        },
    )
    .unwrap();
    assert_eq!(second.items.len(), 31);
    assert!(second.items[0].message.starts_with("29:"));
    assert_eq!(second.items.last().unwrap().message, "oldest");
    assert!(second.next_cursor.is_none());
}

#[test]
fn reads_diagnostics_from_complete_application_events() {
    let directory = tempdir().unwrap();
    let active = directory.path().join("any2api-2026-09-08-000000.jsonl");
    let mut text = event("token refreshed", "INFO");
    text.push_str(&event("OAuth refresh failed", "WARN"));
    text.push_str("invalid line\n{\"timestamp\":");
    fs::write(&active, text).unwrap();
    let query = RuntimeLogQuery::default();
    let page = read_page(directory.path(), &query).unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].message, "OAuth refresh failed");
    assert_eq!(page.items[0].fields["oauth_account_id"], "account-a");
    assert_eq!(page.items[0].fields["reauthorization_required"], "true");
    assert!(
        read_page(
            directory.path(),
            &RuntimeLogQuery {
                cursor: Some("../private.jsonl:0".into()),
            }
        )
        .is_err()
    );
}

#[test]
fn continues_history_when_retention_removes_the_cursor_segment() {
    let directory = tempdir().unwrap();
    fs::write(
        directory.path().join("any2api-2026-09-07-000000.jsonl"),
        event("retained", "ERROR"),
    )
    .unwrap();
    let page = read_page(
        directory.path(),
        &RuntimeLogQuery {
            cursor: Some(format!(
                "{}:any2api-2026-09-08-000000.jsonl:100",
                OffsetDateTime::parse("2026-09-08T02:30:00Z", &Rfc3339)
                    .unwrap()
                    .unix_timestamp_nanos()
            )),
        },
    )
    .unwrap();
    assert_eq!(page.items[0].message, "retained");
}

#[test]
fn reused_segment_numbers_are_sorted_by_their_events() {
    let directory = tempdir().unwrap();
    fs::write(
        directory.path().join("any2api-2026-09-08-000008.jsonl"),
        event("old event", "INFO"),
    )
    .unwrap();
    let new = event("OAuth account token refresh failed", "WARN").replace("02:30:00Z", "03:30:00Z");
    fs::write(
        directory.path().join("any2api-2026-09-08-000000.jsonl"),
        new,
    )
    .unwrap();
    let page = read_page(directory.path(), &RuntimeLogQuery::default()).unwrap();
    assert_eq!(page.items[0].summary, "账号凭据刷新失败");
    assert_eq!(page.items[1].message, "old event");
}
