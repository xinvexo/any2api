use any2api_domain::{ProtocolDialect, ProtocolOperation};
use serde_json::json;

use crate::{ProtocolError, api::MAX_BRIDGE_CONTINUATION_STATE_BYTES};

use super::{bridged_exchange, chat_frame, decoded, registry, upstream_response};

#[tokio::test]
async fn bridge_fails_closed_for_unsupported_or_ambiguous_shapes() {
    let registry = registry();

    for body in [
        json!({"model":"public","input":"hello","unknown":true}),
        json!({"model":"public","input":"hello","n":2}),
        json!({"model":"public","input":"hello","tool_choice":"random"}),
    ] {
        let request = decoded(&registry, ProtocolOperation::Responses, body).await;
        let error = bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(request, "upstream", None)
            .err()
            .expect("unsupported request must fail");
        assert!(matches!(error, ProtocolError::InvalidPayload(_)));
    }

    let lost = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public","input":"hello","previous_response_id":"resp_missing"}),
    )
    .await;
    assert_eq!(
        bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(lost, "upstream", None)
            .err()
            .expect("missing history must fail"),
        ProtocolError::SessionBindingLost
    );

    let oversized = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public",
            "input":"x".repeat(MAX_BRIDGE_CONTINUATION_STATE_BYTES)
        }),
    )
    .await;
    assert!(matches!(
        bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(oversized, "upstream", None),
        Err(ProtocolError::ContinuationTooLarge { .. })
    ));

    assert!(matches!(
        registry.exchange(
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
            ProtocolOperation::ResponsesCompact,
        ),
        Err(ProtocolError::Unsupported(_))
    ));

    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public","input":"hello"}),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    exchange
        .prepare_request(request, "upstream", None)
        .expect("prepared request");
    let error = exchange
        .decode_upstream_response(upstream_response(json!({
            "choices":[
                {"message":{"role":"assistant","content":"one"}},
                {"message":{"role":"assistant","content":"two"}}
            ]
        })))
        .expect_err("multiple choices must fail");
    assert!(matches!(error, ProtocolError::InvalidPayload(_)));

    let stream_request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public","input":"hello","stream":true}),
    )
    .await;
    let mut stream_exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    stream_exchange
        .prepare_request(stream_request, "upstream", None)
        .expect("prepared stream");
    let error = stream_exchange
        .decode_upstream_event(chat_frame(json!({
            "choices":[{"delta":{"content":"one"}},{"delta":{"content":"two"}}]
        })))
        .expect_err("multiple streamed choices must fail");
    assert!(matches!(error, ProtocolError::InvalidPayload(_)));
}
