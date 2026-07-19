use bytes::{Bytes, BytesMut};
use serde_json::{Map, Value};

use crate::{ProtocolError, api::SseFrame};

pub const DEFAULT_MAX_SSE_FRAME_BYTES: usize = 256 * 1024;

#[derive(Debug)]
pub struct SseDecoder {
    buffer: BytesMut,
    max_frame_bytes: usize,
}

impl Default for SseDecoder {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_SSE_FRAME_BYTES)
    }
}

impl SseDecoder {
    #[must_use]
    pub fn new(max_frame_bytes: usize) -> Self {
        Self {
            buffer: BytesMut::new(),
            max_frame_bytes: max_frame_bytes.max(1),
        }
    }

    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseFrame>, ProtocolError> {
        self.buffer.extend_from_slice(chunk);
        self.take_complete_frames()
    }

    pub fn finish(&mut self) -> Result<Option<SseFrame>, ProtocolError> {
        if self.buffer.is_empty() {
            return Ok(None);
        }
        self.check_buffer_limit()?;
        Ok(Some(SseFrame(self.buffer.split().freeze())))
    }

    fn take_complete_frames(&mut self) -> Result<Vec<SseFrame>, ProtocolError> {
        let mut frames = Vec::new();
        while let Some(end) = find_event_end(&self.buffer) {
            if end > self.max_frame_bytes {
                return Err(frame_limit_error());
            }
            frames.push(SseFrame(self.buffer.split_to(end).freeze()));
        }
        self.check_buffer_limit()?;
        Ok(frames)
    }

    fn check_buffer_limit(&self) -> Result<(), ProtocolError> {
        if self.buffer.len() > self.max_frame_bytes {
            Err(frame_limit_error())
        } else {
            Ok(())
        }
    }
}

fn frame_limit_error() -> ProtocolError {
    ProtocolError::InvalidPayload("SSE frame exceeded the configured limit".into())
}

fn find_event_end(bytes: &[u8]) -> Option<usize> {
    for index in 0..bytes.len() {
        match bytes[index] {
            b'\n' if bytes.get(index + 1) == Some(&b'\n') => return Some(index + 2),
            b'\n'
                if bytes.get(index + 1) == Some(&b'\r') && bytes.get(index + 2) == Some(&b'\n') =>
            {
                return Some(index + 3);
            }
            b'\r'
                if bytes.get(index + 1) == Some(&b'\n')
                    && bytes.get(index + 2) == Some(&b'\r')
                    && bytes.get(index + 3) == Some(&b'\n') =>
            {
                return Some(index + 4);
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn rewrite_known_model(
    frame: SseFrame,
    public_model: &str,
) -> Result<SseFrame, ProtocolError> {
    let original = frame.0.clone();
    let text = std::str::from_utf8(&original)
        .map_err(|_| ProtocolError::InvalidPayload("SSE frame is not valid UTF-8".into()))?;
    let normalized = text.replace("\r\n", "\n");
    let lines = normalized
        .trim_end_matches('\n')
        .split('\n')
        .collect::<Vec<_>>();
    let data = lines
        .iter()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|value| value.strip_prefix(' ').unwrap_or(value))
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() || data.trim() == "[DONE]" {
        return Ok(SseFrame(original));
    }
    let Ok(mut value) = serde_json::from_str::<Value>(&data) else {
        return Ok(SseFrame(original));
    };
    if !rewrite_model_fields(&mut value, public_model) {
        return Ok(SseFrame(original));
    }
    let serialized = serde_json::to_string(&value)
        .map_err(|_| ProtocolError::InvalidPayload("SSE event could not be encoded".into()))?;
    let mut output = String::new();
    let mut replaced = false;
    for line in lines {
        if line.starts_with("data:") {
            if !replaced {
                output.push_str("data: ");
                output.push_str(&serialized);
                output.push('\n');
                replaced = true;
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output.push('\n');
    Ok(SseFrame(Bytes::from(output)))
}

fn rewrite_model_fields(value: &mut Value, public_model: &str) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let mut changed = replace_model(object, public_model);
    for container in ["response", "message"] {
        if let Some(Value::Object(nested)) = object.get_mut(container) {
            changed |= replace_model(nested, public_model);
        }
    }
    changed
}

fn replace_model(object: &mut Map<String, Value>, public_model: &str) -> bool {
    let Some(Value::String(model)) = object.get_mut("model") else {
        return false;
    };
    if model == public_model {
        false
    } else {
        *model = public_model.to_owned();
        true
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::{SseDecoder, rewrite_known_model};
    use crate::api::SseFrame;

    #[test]
    fn decoder_handles_arbitrary_chunks_crlf_and_multiline_data() {
        let mut decoder = SseDecoder::new(1024);
        let mut frames = Vec::new();
        for chunk in [
            b"event: test\r".as_slice(),
            b"\ndata: first\r\n".as_slice(),
            b"data: second\r\n\r".as_slice(),
            b"\nevent: done\ndata: [DONE]\n\n".as_slice(),
        ] {
            frames.extend(decoder.push(chunk).expect("SSE chunk"));
        }
        assert_eq!(frames.len(), 2);
        assert_eq!(
            frames[0].0,
            Bytes::from_static(b"event: test\r\ndata: first\r\ndata: second\r\n\r\n")
        );
        assert_eq!(
            frames[1].0,
            Bytes::from_static(b"event: done\ndata: [DONE]\n\n")
        );
    }

    #[test]
    fn decoder_flushes_an_eof_frame_without_a_trailing_blank_line() {
        let mut decoder = SseDecoder::default();
        assert!(
            decoder
                .push(b"data: {\"ok\":true}")
                .expect("chunk")
                .is_empty()
        );
        let frame = decoder.finish().expect("finish").expect("final frame");
        assert_eq!(frame.0, Bytes::from_static(b"data: {\"ok\":true}"));
    }

    #[test]
    fn decoder_rejects_a_frame_larger_than_its_limit() {
        let mut decoder = SseDecoder::new(8);
        let error = decoder
            .push(b"data: oversized\n\n")
            .expect_err("oversized frame must fail");

        assert!(error.to_string().contains("configured limit"));
    }

    #[test]
    fn model_rewrite_only_touches_known_response_containers() {
        let frame = SseFrame(Bytes::from_static(
            b"event: response.created\ndata: {\"response\":{\"model\":\"upstream\"},\"model\":\"upstream\",\"metadata\":{\"model\":\"keep\"}}\n\n",
        ));
        let rewritten = rewrite_known_model(frame, "public").expect("rewrite");
        let text = std::str::from_utf8(&rewritten.0).expect("UTF-8 event");
        let data = text
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .expect("data field");
        let value: serde_json::Value = serde_json::from_str(data).expect("event JSON");
        assert_eq!(value["model"], "public");
        assert_eq!(value["response"]["model"], "public");
        assert_eq!(value["metadata"]["model"], "keep");
    }

    #[test]
    fn model_rewrite_preserves_done_and_non_json_events() {
        let done = SseFrame(Bytes::from_static(b"data: [DONE]\n\n"));
        assert_eq!(
            rewrite_known_model(done.clone(), "public").expect("done"),
            done
        );
        let text = SseFrame(Bytes::from_static(b"data: plain text\n\n"));
        assert_eq!(
            rewrite_known_model(text.clone(), "public").expect("text"),
            text
        );
    }
}
