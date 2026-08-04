use std::ops::Range;

use bytes::Bytes;
use serde::de::IgnoredAny;
use serde_json::Value;

use crate::{ProtocolError, api::SseEventPayload};

/// A validated JSON `data:` payload that shares the upstream frame allocation
/// whenever the event carried a single `data:` line.
#[derive(Clone, Eq, PartialEq)]
pub struct SseJsonData {
    event_name: Option<Bytes>,
    data: Bytes,
}

impl SseJsonData {
    pub(crate) fn new(event_name: Option<Bytes>, data: Bytes) -> Self {
        Self { event_name, data }
    }

    #[must_use]
    pub fn event_name(&self) -> Option<&str> {
        self.event_name
            .as_deref()
            .and_then(|name| std::str::from_utf8(name).ok())
    }

    #[must_use]
    pub fn data(&self) -> &Bytes {
        &self.data
    }

    /// Materialize the full JSON tree. Bridges that transform every field pay
    /// this once; field probes should use the `raw_json` scanners instead.
    pub fn to_value(&self) -> Result<Value, ProtocolError> {
        serde_json::from_slice(&self.data)
            .map_err(|_| ProtocolError::InvalidPayload("SSE event data is not valid JSON".into()))
    }
}

/// Classify a complete SSE frame without copying it: field lines are scanned
/// in place, the last `event:` field wins, and a lone `data:` line is borrowed
/// straight from the frame allocation.
pub(crate) fn parse_event_payload(frame: &Bytes) -> SseEventPayload {
    let bytes = frame.as_ref();
    let mut event_name = None;
    let mut first_data: Option<Range<usize>> = None;
    let mut extra_data = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let (line, next) = next_line(bytes, offset);
        offset = next;
        match field_value(bytes, line) {
            Some((b"event", value)) => event_name = Some(value),
            Some((b"data", value)) => {
                if first_data.is_none() {
                    first_data = Some(value);
                } else {
                    extra_data.push(value);
                }
            }
            _ => {}
        }
    }
    let Some(first) = first_data else {
        return SseEventPayload::Empty;
    };
    let event_name = event_name.map(|range| frame.slice(range));
    if extra_data.is_empty() {
        if first.is_empty() {
            return SseEventPayload::Empty;
        }
        return classify_data(event_name, frame.slice(first));
    }
    let mut joined = Vec::with_capacity(
        extra_data
            .iter()
            .fold(first.len() + extra_data.len(), |sum, line| sum + line.len()),
    );
    joined.extend_from_slice(&bytes[first]);
    for line in extra_data {
        joined.push(b'\n');
        joined.extend_from_slice(&bytes[line]);
    }
    classify_data(event_name, Bytes::from(joined))
}

fn classify_data(event_name: Option<Bytes>, data: Bytes) -> SseEventPayload {
    if data.trim_ascii() == b"[DONE]" {
        return SseEventPayload::Done;
    }
    // Validate via `&str` so JSON strings holding invalid UTF-8 are rejected
    // here instead of surfacing later from a downstream field probe.
    let Ok(text) = std::str::from_utf8(&data) else {
        return SseEventPayload::NonJson;
    };
    if serde_json::from_str::<IgnoredAny>(text).is_err() {
        return SseEventPayload::NonJson;
    }
    SseEventPayload::Json(SseJsonData::new(event_name, data))
}

/// Split `field: value` per the SSE spec: at most one leading space of the
/// value is removed, a line without a colon is a field with an empty value,
/// and comment or blank lines yield no field at all.
fn field_value(bytes: &[u8], line: Range<usize>) -> Option<(&[u8], Range<usize>)> {
    let text = &bytes[line.clone()];
    if text.is_empty() || text[0] == b':' {
        return None;
    }
    let (name_end, mut value_start) = match text.iter().position(|byte| *byte == b':') {
        Some(colon) => (colon, colon + 1),
        None => (text.len(), text.len()),
    };
    if text.get(value_start) == Some(&b' ') {
        value_start += 1;
    }
    Some((
        &bytes[line.start..line.start + name_end],
        line.start + value_start..line.end,
    ))
}

/// Return one line (without its terminator) plus the offset after the
/// terminator, accepting `\n`, `\r\n`, and bare `\r` endings in place.
pub(super) fn next_line(bytes: &[u8], start: usize) -> (Range<usize>, usize) {
    let mut index = start;
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => return (start..index, index + 1),
            b'\r' => {
                let next = if bytes.get(index + 1) == Some(&b'\n') {
                    index + 2
                } else {
                    index + 1
                };
                return (start..index, next);
            }
            _ => index += 1,
        }
    }
    (start..bytes.len(), bytes.len())
}
