use bytes::Bytes;
use serde_json::json;

use super::super::wire::{encoding_count, reset_encoding_count};
use super::ChatToResponsesStream;
use crate::api::{AdapterEvent, ProtocolEventTelemetry, SseEventPayload, SseJsonData};
use crate::openai_responses_chat::{
    response_projection::ResponseProjection, tool_projection::ToolProjection,
};
use any2api_domain::OpenAiChatCompletionsProfile;

#[test]
fn synthesized_events_are_encoded_once_after_sequence_injection() {
    reset_encoding_count();
    let profile = OpenAiChatCompletionsProfile::COMPATIBLE_BASELINE;
    let mut stream = ChatToResponsesStream::new(
        ResponseProjection::new(
            "resp_test".to_owned(),
            "model".to_owned(),
            &json!({"model":"public","input":"hello"}),
        ),
        profile,
        ToolProjection::new(profile, None),
    );
    let upstream = AdapterEvent::new(
        Bytes::from_static(b"upstream frame"),
        ProtocolEventTelemetry::default(),
        SseEventPayload::Json(SseJsonData::new(
            None,
            Bytes::from(
                serde_json::to_vec(&json!({
                    "model":"model",
                    "choices":[{"index":0,"delta":{"content":"hello"}}]
                }))
                .expect("event JSON"),
            ),
        )),
    );

    let update = stream.push(upstream).expect("translated stream update");

    assert_eq!(update.events.len(), 5);
    assert_eq!(encoding_count(), update.events.len());
    assert_eq!(
        update.events.last().expect("text delta").bytes().as_ref(),
        b"event: response.output_text.delta\ndata: {\"content_index\":0,\"delta\":\"hello\",\"item_id\":\"msg_resp_test\",\"output_index\":0,\"sequence_number\":4,\"type\":\"response.output_text.delta\"}\n\n"
    );
}
