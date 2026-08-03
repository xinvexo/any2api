use any2api_domain::ProtocolOperation;
use serde_json::{Value, json};

use crate::api::{
    BridgeContinuationState, PreparedProtocolRequest, ProtocolContinuationState, ProtocolExchange,
};

use super::{bridged_exchange, decoded, registry, upstream_response};

#[tokio::test]
async fn continuation_applies_only_the_current_turn_instructions() {
    let registry = registry();
    let first = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public-model",
            "instructions":"Be concise",
            "input":"turn one"
        }),
    )
    .await;
    let mut first_exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let first_prepared = first_exchange
        .prepare_request(&first, "upstream-model", None)
        .expect("first request");
    assert_eq!(
        messages(&first_prepared),
        vec![
            json!({"role":"system","content":"Be concise"}),
            json!({"role":"user","content":"turn one"}),
        ]
    );
    let (first_id, first_continuation) = complete_turn(&mut first_exchange, "answer one");

    let second = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public-model",
            "previous_response_id":first_id,
            "instructions":"Be concise",
            "input":"turn two"
        }),
    )
    .await;
    let mut second_exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let second_prepared = second_exchange
        .prepare_request(&second, "upstream-model", Some(first_continuation))
        .expect("second request");
    assert_eq!(
        messages(&second_prepared),
        vec![
            json!({"role":"system","content":"Be concise"}),
            json!({"role":"user","content":"turn one"}),
            json!({"role":"assistant","content":"answer one"}),
            json!({"role":"user","content":"turn two"}),
        ]
    );
    let (second_id, second_continuation) = complete_turn(&mut second_exchange, "answer two");

    let third = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public-model",
            "previous_response_id":second_id,
            "instructions":"Return JSON",
            "input":"turn three"
        }),
    )
    .await;
    let mut third_exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let third_prepared = third_exchange
        .prepare_request(&third, "upstream-model", Some(second_continuation))
        .expect("third request");
    assert_eq!(
        messages(&third_prepared),
        vec![
            json!({"role":"system","content":"Return JSON"}),
            json!({"role":"user","content":"turn one"}),
            json!({"role":"assistant","content":"answer one"}),
            json!({"role":"user","content":"turn two"}),
            json!({"role":"assistant","content":"answer two"}),
            json!({"role":"user","content":"turn three"}),
        ]
    );
}

fn messages(prepared: &PreparedProtocolRequest) -> Vec<Value> {
    let body: Value =
        serde_json::from_slice(&prepared.request.body).expect("upstream request JSON");
    body["messages"]
        .as_array()
        .expect("Chat Completions messages")
        .clone()
}

fn complete_turn(
    exchange: &mut ProtocolExchange,
    assistant_text: &str,
) -> (String, ProtocolContinuationState) {
    let response = exchange
        .decode_upstream_response(upstream_response(json!({
            "created":1,
            "model":"upstream-model",
            "choices":[{
                "index":0,
                "message":{"role":"assistant","content":assistant_text},
                "finish_reason":"stop"
            }]
        })))
        .expect("bridged response");
    let response_id = exchange
        .continuation_id_from_response(ProtocolOperation::Responses, &response)
        .expect("response identity")
        .expect("response id");
    let continuation = match exchange.bridge_continuation_state() {
        BridgeContinuationState::Ready(state) => state,
        other => panic!("buffered continuation must be ready: {other:?}"),
    };
    (response_id, continuation)
}
