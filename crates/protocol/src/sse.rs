use bytes::{Bytes, BytesMut};
use serde_json::{Map, Value};

use crate::{
    ProtocolError,
    api::{SseEventPayload, SseFrame},
};

#[derive(Debug)]
pub struct SseDecoder {
    buffer: BytesMut,
    max_frame_bytes: usize,
    scan_from: usize,
}

impl SseDecoder {
    #[must_use]
    pub fn new(max_frame_bytes: usize) -> Self {
        Self {
            buffer: BytesMut::new(),
            max_frame_bytes: max_frame_bytes.max(1),
            scan_from: 0,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
    }

    pub fn next_input_limit(&self) -> usize {
        self.max_frame_bytes
            .saturating_sub(self.buffer.len())
            .saturating_add(1)
    }

    pub fn next_frame(&mut self) -> Result<Option<SseFrame>, ProtocolError> {
        let Some(end) = find_event_end(&self.buffer, self.scan_from) else {
            self.scan_from = self.buffer.len().saturating_sub(3);
            self.check_buffer_limit()?;
            return Ok(None);
        };
        if end > self.max_frame_bytes {
            return Err(frame_limit_error());
        }
        self.scan_from = 0;
        Ok(Some(SseFrame(self.buffer.split_to(end).freeze())))
    }

    pub fn finish(&mut self) -> Result<Option<SseFrame>, ProtocolError> {
        if self.buffer.is_empty() {
            return Ok(None);
        }
        self.check_buffer_limit()?;
        Ok(Some(SseFrame(self.buffer.split().freeze())))
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

fn find_event_end(bytes: &[u8], start: usize) -> Option<usize> {
    for index in start.min(bytes.len())..bytes.len() {
        let Some(first) = line_ending_len(bytes, index) else {
            continue;
        };
        let second_at = index + first;
        if let Some(second) = line_ending_len(bytes, second_at) {
            return Some(second_at + second);
        }
    }
    None
}

fn line_ending_len(bytes: &[u8], index: usize) -> Option<usize> {
    match *bytes.get(index)? {
        b'\n' => Some(1),
        b'\r' => match bytes.get(index + 1) {
            Some(b'\n') => Some(2),
            Some(_) => Some(1),
            None => None,
        },
        _ => None,
    }
}

pub(crate) fn parse_event_payload(bytes: &[u8]) -> Result<SseEventPayload, ProtocolError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ProtocolError::InvalidPayload("SSE frame is not valid UTF-8".into()))?;
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let event_name = normalized
        .lines()
        .find_map(|line| line.strip_prefix("event:"))
        .map(str::trim)
        .map(str::to_owned);
    let data = normalized
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() {
        return Ok(SseEventPayload::Empty);
    }
    if data.trim() == "[DONE]" {
        return Ok(SseEventPayload::Done);
    }
    match serde_json::from_str(&data) {
        Ok(data) => Ok(SseEventPayload::Json { event_name, data }),
        Err(_) => Ok(SseEventPayload::NonJson),
    }
}

pub(crate) fn rewrite_known_model(
    bytes: Bytes,
    payload: SseEventPayload,
    public_model: &str,
) -> Result<SseFrame, ProtocolError> {
    let SseEventPayload::Json {
        data: mut value, ..
    } = payload
    else {
        return Ok(SseFrame(bytes));
    };
    if !rewrite_model_fields(&mut value, public_model) {
        return Ok(SseFrame(bytes));
    }
    let serialized = serde_json::to_string(&value)
        .map_err(|_| ProtocolError::InvalidPayload("SSE event could not be encoded".into()))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| ProtocolError::InvalidPayload("SSE frame is not valid UTF-8".into()))?;
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut output = String::new();
    let mut replaced = false;
    for line in normalized.trim_end_matches('\n').split('\n') {
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

    use super::{SseDecoder, parse_event_payload, rewrite_known_model};
    use crate::api::SseFrame;

    fn rewrite(frame: SseFrame, public_model: &str) -> SseFrame {
        let payload = parse_event_payload(&frame.0).expect("payload");
        rewrite_known_model(frame.0, payload, public_model).expect("rewrite")
    }

    #[test]
    fn decoder_handles_arbitrary_chunks_all_line_endings_and_multiline_data() {
        let mut decoder = SseDecoder::new(1024);
        let mut frames = Vec::new();
        for chunk in [
            b"event: test\r".as_slice(),
            b"data: [1,\rdata: 2]\r\r".as_slice(),
            b"event: done\r".as_slice(),
            b"\ndata: [DONE]\r\n\r".as_slice(),
            b"\n".as_slice(),
        ] {
            decoder.push(chunk);
            while let Some(frame) = decoder.next_frame().expect("SSE frame") {
                frames.push(frame);
            }
        }
        assert_eq!(frames.len(), 2);
        assert_eq!(
            frames[0].0,
            Bytes::from_static(b"event: test\rdata: [1,\rdata: 2]\r\r")
        );
        assert_eq!(
            frames[1].0,
            Bytes::from_static(b"event: done\r\ndata: [DONE]\r\n\r\n")
        );
        assert!(matches!(
            parse_event_payload(&frames[0].0),
            Ok(crate::api::SseEventPayload::Json { event_name: Some(name), data })
                if name == "test" && data == serde_json::json!([1, 2])
        ));
    }

    #[test]
    fn decoder_handles_single_byte_chunks() {
        let mut decoder = SseDecoder::new(1024);
        let mut frames = Vec::new();
        for byte in b"data: one\n\ndata: two\n\n" {
            decoder.push(std::slice::from_ref(byte));
            while let Some(frame) = decoder.next_frame().expect("SSE frame") {
                frames.push(frame);
            }
        }

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].0, Bytes::from_static(b"data: one\n\n"));
        assert_eq!(frames[1].0, Bytes::from_static(b"data: two\n\n"));
    }

    #[test]
    fn decoder_flushes_an_eof_frame_without_a_trailing_blank_line() {
        let mut decoder = SseDecoder::new(1024);
        decoder.push(b"data: {\"ok\":true}");
        assert!(decoder.next_frame().expect("frame").is_none());
        let frame = decoder.finish().expect("finish").expect("final frame");
        assert_eq!(frame.0, Bytes::from_static(b"data: {\"ok\":true}"));
    }

    #[test]
    fn decoder_rejects_a_frame_larger_than_its_limit() {
        let mut decoder = SseDecoder::new(8);
        decoder.push(b"data: oversized\n\n");
        let error = decoder.next_frame().expect_err("oversized frame must fail");

        assert!(error.to_string().contains("configured limit"));
    }

    #[test]
    fn decoder_preserves_complete_frames_before_a_later_limit_error() {
        let mut decoder = SseDecoder::new(12);
        decoder.push(b"data: ok\n\ndata: oversized\n\n");
        let frame = decoder
            .next_frame()
            .expect("first frame")
            .expect("complete first frame");
        let error = decoder
            .next_frame()
            .expect_err("later oversized frame must fail");

        assert_eq!(frame, SseFrame(Bytes::from_static(b"data: ok\n\n")));
        assert!(error.to_string().contains("configured limit"));
    }

    #[test]
    fn model_rewrite_only_touches_known_response_containers() {
        let frame = SseFrame(Bytes::from_static(
            b"event: response.created\ndata: {\"response\":{\"model\":\"upstream\"},\"model\":\"upstream\",\"metadata\":{\"model\":\"keep\"}}\n\n",
        ));
        let rewritten = rewrite(frame, "public");
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
        assert_eq!(rewrite(done.clone(), "public"), done);
        let text = SseFrame(Bytes::from_static(b"data: plain text\n\n"));
        assert_eq!(rewrite(text.clone(), "public"), text);
    }

    #[test]
    fn payload_parser_distinguishes_done_from_empty_heartbeats() {
        assert_eq!(
            parse_event_payload(b"data: [DONE]\n\n").expect("done payload"),
            crate::api::SseEventPayload::Done
        );
        assert_eq!(
            parse_event_payload(b": keep-alive\n\n").expect("heartbeat payload"),
            crate::api::SseEventPayload::Empty
        );
    }

    mod properties {
        use proptest::prelude::*;

        use super::super::{SseDecoder, parse_event_payload};
        use crate::api::SseEventPayload;

        /// Feed `input` to a decoder in the given chunk sizes and collect
        /// every frame plus the EOF remainder.
        fn decode_chunked(input: &[u8], chunk_sizes: &[usize]) -> Vec<Vec<u8>> {
            let mut decoder = SseDecoder::new(1 << 20);
            let mut frames = Vec::new();
            let mut rest = input;
            let mut sizes = chunk_sizes.iter().copied().cycle();
            while !rest.is_empty() {
                let take = sizes.next().unwrap_or(1).clamp(1, rest.len());
                let (chunk, remaining) = rest.split_at(take);
                rest = remaining;
                decoder.push(chunk);
                while let Some(frame) = decoder.next_frame().expect("frame within limit") {
                    frames.push(frame.0.to_vec());
                }
            }
            if let Some(frame) = decoder.finish().expect("finish within limit") {
                frames.push(frame.0.to_vec());
            }
            frames
        }

        proptest! {
            /// Frames must not depend on how the byte stream is chunked, and
            /// reassembling them must reproduce the input losslessly.
            #[test]
            fn chunking_never_changes_frames_and_reassembly_is_lossless(
                input in proptest::collection::vec(
                    prop_oneof![
                        Just(b'\n'), Just(b'\r'), Just(b':'), Just(b' '),
                        any::<u8>(),
                    ],
                    0..200,
                ),
                chunk_sizes in proptest::collection::vec(1_usize..17, 1..8),
            ) {
                let chunked = decode_chunked(&input, &chunk_sizes);
                let whole = decode_chunked(&input, &[input.len().max(1)]);
                prop_assert_eq!(&chunked, &whole);
                let reassembled = chunked.concat();
                prop_assert_eq!(reassembled, input);
            }

            /// The payload parser must never panic and must classify every
            /// frame into exactly one of the four payload shapes.
            #[test]
            fn payload_parse_is_total_over_arbitrary_frames(
                input in proptest::collection::vec(any::<u8>(), 0..300),
            ) {
                match parse_event_payload(&input) {
                    Ok(
                        SseEventPayload::Empty
                        | SseEventPayload::Done
                        | SseEventPayload::NonJson,
                    ) => {}
                    Ok(SseEventPayload::Json { .. }) => {}
                    Err(_) => prop_assert!(
                        std::str::from_utf8(&input).is_err(),
                        "parse may only fail on invalid UTF-8",
                    ),
                }
            }
        }
    }
}
