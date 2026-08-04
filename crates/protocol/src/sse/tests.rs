use bytes::Bytes;

use super::{SseDecoder, parse_event_payload, rewrite_known_model};
use crate::api::{SseEventPayload, SseFrame};

fn rewrite(frame: SseFrame, public_model: &str) -> SseFrame {
    let payload = parse_event_payload(&frame.0);
    rewrite_known_model(frame.0, payload, public_model).expect("rewrite")
}

fn data_json(frame: &SseFrame) -> serde_json::Value {
    match parse_event_payload(&frame.0) {
        SseEventPayload::Json(data) => data.to_value().expect("event JSON"),
        other => panic!("expected JSON payload, got {other:?}"),
    }
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
    match parse_event_payload(&frames[0].0) {
        SseEventPayload::Json(data) => {
            assert_eq!(data.event_name(), Some("test"));
            assert_eq!(
                data.to_value().expect("data JSON"),
                serde_json::json!([1, 2])
            );
        }
        other => panic!("expected JSON payload, got {other:?}"),
    }
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
    let value = data_json(&rewrite(frame, "public"));
    assert_eq!(value["model"], "public");
    assert_eq!(value["response"]["model"], "public");
    assert_eq!(value["metadata"]["model"], "keep");
}

#[test]
fn model_rewrite_splices_in_place_keeping_key_order_numbers_and_line_endings() {
    let frame = SseFrame(Bytes::from_static(
        b"event: chunk\r\ndata: {\"z\":9007199254740993,\"model\":\"upstream\",\"a\":18446744073709551615,\"response\":{\"big\":10.100,\"model\":\"upstream\"}}\r\n\r\n",
    ));
    let rewritten = rewrite(frame, "public");
    assert_eq!(
        rewritten.0,
        Bytes::from_static(
            b"event: chunk\r\ndata: {\"z\":9007199254740993,\"model\":\"public\",\"a\":18446744073709551615,\"response\":{\"big\":10.100,\"model\":\"public\"}}\r\n\r\n",
        )
    );
}

#[test]
fn model_rewrite_leaves_matching_models_byte_identical() {
    let frame = SseFrame(Bytes::from_static(
        b"data: {\"model\":\"public\",\"big\":9007199254740993}\n\n",
    ));
    let rewritten = rewrite(frame.clone(), "public");
    assert_eq!(rewritten.0.as_ptr(), frame.0.as_ptr());
}

#[test]
fn model_rewrite_supports_multiline_data_frames() {
    let frame = SseFrame(Bytes::from_static(
        b"event: chunk\ndata: {\"model\":\ndata: \"upstream\",\"keep\":1}\n\n",
    ));
    let value = data_json(&rewrite(frame, "public"));
    assert_eq!(value["model"], "public");
    assert_eq!(value["keep"], 1);
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
        parse_event_payload(&Bytes::from_static(b"data: [DONE]\n\n")),
        SseEventPayload::Done
    );
    assert_eq!(
        parse_event_payload(&Bytes::from_static(b": keep-alive\n\n")),
        SseEventPayload::Empty
    );
    assert_eq!(
        parse_event_payload(&Bytes::from_static(b"data: \n\n")),
        SseEventPayload::Empty
    );
}

#[test]
fn payload_parser_follows_sse_field_semantics() {
    let frame = Bytes::from_static(b"event: first\nevent: last \ndata:  {\"ok\":1}\n\n");
    match parse_event_payload(&frame) {
        SseEventPayload::Json(data) => {
            assert_eq!(data.event_name(), Some("last "));
            assert_eq!(data.data().as_ref(), b" {\"ok\":1}");
        }
        other => panic!("expected JSON payload, got {other:?}"),
    }
    assert_eq!(
        parse_event_payload(&Bytes::from_static(b"data\ndata\n\n")),
        SseEventPayload::NonJson
    );
}

#[test]
fn payload_parser_reports_invalid_utf8_as_non_json_without_failing_the_stream() {
    assert_eq!(
        parse_event_payload(&Bytes::from_static(b"data: \"\xff\"\n\n")),
        SseEventPayload::NonJson
    );
}

mod properties {
    use bytes::Bytes;
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

        /// The payload parser must never fail, must classify every frame into
        /// exactly one of the four payload shapes, and must only report `Json`
        /// when the data really is valid JSON.
        #[test]
        fn payload_parse_is_total_over_arbitrary_frames(
            input in proptest::collection::vec(any::<u8>(), 0..300),
        ) {
            match parse_event_payload(&Bytes::from(input)) {
                SseEventPayload::Empty
                | SseEventPayload::Done
                | SseEventPayload::NonJson => {}
                SseEventPayload::Json(data) => {
                    prop_assert!(data.to_value().is_ok(), "Json payloads must reparse");
                }
            }
        }
    }
}
