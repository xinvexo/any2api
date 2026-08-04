use std::ops::Range;

use bytes::Bytes;

use crate::{
    ProtocolError,
    api::{SseEventPayload, SseFrame},
    raw_json::{json_string, scan_object_fields, splice_ranges, subslice_range},
};

use super::event::next_line;

/// Rewrite the upstream model name inside a decoded event by splicing the
/// affected byte ranges of the original frame; every other byte — key order,
/// number formatting, line endings — passes through untouched.
pub(crate) fn rewrite_known_model(
    bytes: Bytes,
    payload: SseEventPayload,
    public_model: &str,
) -> Result<SseFrame, ProtocolError> {
    let SseEventPayload::Json(data) = &payload else {
        return Ok(SseFrame(bytes));
    };
    let spans = model_spans(data.data(), public_model);
    if spans.is_empty() {
        return Ok(SseFrame(bytes));
    }
    let replacement = serde_json::to_string(public_model)
        .map_err(|_| ProtocolError::InvalidPayload("SSE event could not be encoded".into()))?;
    if let Some(range) = subslice_range(&bytes, data.data()) {
        let frame_spans = spans
            .iter()
            .map(|span| range.start + span.start..range.start + span.end)
            .collect::<Vec<_>>();
        return Ok(SseFrame(splice_ranges(
            &bytes,
            &frame_spans,
            replacement.as_bytes(),
        )));
    }
    let spliced = splice_ranges(data.data(), &spans, replacement.as_bytes());
    Ok(SseFrame(rebuild_frame(&bytes, &spliced)))
}

/// Byte ranges of every `model` string value that must change: the top-level
/// field plus the known `response`/`message` containers, ordered ascending.
fn model_spans(data: &[u8], public_model: &str) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut containers = Vec::new();
    scan_object_fields(
        data,
        &["model", "response", "message"],
        &mut |index, value| {
            if index == 0 {
                push_model_span(data, value, public_model, &mut spans);
            } else {
                containers.push(value);
            }
        },
    );
    for container in containers {
        scan_object_fields(container.get().as_bytes(), &["model"], &mut |_, value| {
            push_model_span(data, value, public_model, &mut spans);
        });
    }
    spans.sort_unstable_by_key(|span| span.start);
    spans
}

fn push_model_span(
    data: &[u8],
    value: &serde_json::value::RawValue,
    public_model: &str,
    spans: &mut Vec<Range<usize>>,
) {
    let Some(model) = json_string(value) else {
        return;
    };
    if model != public_model
        && let Some(range) = subslice_range(data, value.get().as_bytes())
    {
        spans.push(range);
    }
}

/// Rare path for events whose data spanned several `data:` lines: the joined
/// buffer no longer aliases the frame, so re-emit the frame with the spliced
/// data split back into lines.
fn rebuild_frame(bytes: &[u8], data: &[u8]) -> Bytes {
    let mut output = Vec::with_capacity(bytes.len() + data.len());
    let mut data_written = false;
    let mut offset = 0;
    while offset < bytes.len() {
        let (line, next) = next_line(bytes, offset);
        offset = next;
        let line = &bytes[line];
        if is_data_line(line) {
            if !data_written {
                data_written = true;
                for piece in data.split(|byte| *byte == b'\n') {
                    output.extend_from_slice(b"data: ");
                    output.extend_from_slice(piece);
                    output.push(b'\n');
                }
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        output.extend_from_slice(line);
        output.push(b'\n');
    }
    output.push(b'\n');
    Bytes::from(output)
}

fn is_data_line(line: &[u8]) -> bool {
    line == b"data" || line.starts_with(b"data:")
}
