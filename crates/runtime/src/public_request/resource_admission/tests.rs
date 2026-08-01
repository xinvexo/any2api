use any2api_domain::ProtocolOperation;
use any2api_protocol::api::{IngressAffinity, MAX_BRIDGE_CONTINUATION_STATE_BYTES};
use bytes::Bytes;
use futures_util::{StreamExt, stream};
use http::{HeaderMap, StatusCode};
use std::{sync::mpsc, time::Duration};

use super::{
    MEMORY_UNIT_BYTES, PUBLIC_REQUEST_MEMORY_BUDGET_BYTES, PublicRequestAdmissionError,
    PublicRequestMemoryBudget, hold_response_memory, request_parse_bytes,
};
use crate::public_request::{PublicResponse, PublicResponseBody};

#[test]
fn image_limits_fit_one_worst_case_request_in_the_fixed_budget() {
    let body = super::execution_limits::request_body_limit(ProtocolOperation::ImagesEdits);
    let response =
        super::execution_limits::buffered_response_limit(ProtocolOperation::ImagesEdits, true);
    assert_eq!(
        request_parse_bytes(body),
        PUBLIC_REQUEST_MEMORY_BUDGET_BYTES
    );
    assert_eq!(body + response * 3, PUBLIC_REQUEST_MEMORY_BUDGET_BYTES);
}

#[test]
fn admissions_share_capacity_and_release_it_on_drop() {
    let budget = PublicRequestMemoryBudget::with_capacity_bytes(8 * MEMORY_UNIT_BYTES);
    let first = budget
        .try_admit_body(ProtocolOperation::Responses, MEMORY_UNIT_BYTES)
        .expect("first admission");
    let second = budget
        .try_admit_body(ProtocolOperation::Responses, MEMORY_UNIT_BYTES)
        .expect("second admission");
    assert!(matches!(
        budget.try_admit_body(ProtocolOperation::Responses, 1),
        Err(PublicRequestAdmissionError::Capacity)
    ));
    drop(first);
    assert!(
        budget
            .try_admit_body(ProtocolOperation::Responses, 1)
            .is_ok()
    );
    drop(second);
}

#[test]
fn execution_expansion_fails_atomically_and_can_retry_after_release() {
    let budget = PublicRequestMemoryBudget::with_capacity_bytes(4 * MEMORY_UNIT_BYTES);
    let mut request = budget
        .try_admit_body(ProtocolOperation::Responses, 1)
        .expect("request admission");
    let blocker = budget
        .try_admit_body(ProtocolOperation::Responses, 1)
        .expect("blocking admission");

    assert_eq!(
        request.reserve_execution(MEMORY_UNIT_BYTES, &IngressAffinity::None),
        Err(PublicRequestAdmissionError::Capacity)
    );
    drop(blocker);
    request
        .reserve_execution(MEMORY_UNIT_BYTES, &IngressAffinity::None)
        .expect("released capacity is reusable");
}

#[test]
fn continuation_execution_reserves_one_full_opaque_state_before_routing() {
    let execution_bytes = MEMORY_UNIT_BYTES;
    let response_peak = 1 + execution_bytes * 3;
    let capacity = response_peak + MAX_BRIDGE_CONTINUATION_STATE_BYTES;
    let budget = PublicRequestMemoryBudget::with_capacity_bytes(capacity);
    let mut request = budget
        .try_admit_body(ProtocolOperation::Responses, 1)
        .expect("continuation request admission");

    request
        .reserve_execution(
            execution_bytes,
            &IngressAffinity::Continuation("resp-private".into()),
        )
        .expect("continuation working set fits exactly");

    assert!(matches!(
        budget.try_admit_body(ProtocolOperation::Responses, 1),
        Err(PublicRequestAdmissionError::Capacity)
    ));
    drop(request);
    assert!(
        budget
            .try_admit_body(ProtocolOperation::Responses, 1)
            .is_ok()
    );
}

#[test]
fn continuation_working_set_stays_reserved_until_stream_drop() {
    let execution_bytes = MEMORY_UNIT_BYTES;
    let response_peak = 1 + execution_bytes * 3;
    let capacity = response_peak + MAX_BRIDGE_CONTINUATION_STATE_BYTES;
    let budget = PublicRequestMemoryBudget::with_capacity_bytes(capacity);
    let mut admission = budget
        .try_admit_body(ProtocolOperation::Responses, 1)
        .expect("continuation request admission");
    admission
        .reserve_execution(
            execution_bytes,
            &IngressAffinity::Continuation("resp-private".into()),
        )
        .expect("continuation working set fits exactly");

    let response = hold_response_memory(
        admission,
        PublicResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: PublicResponseBody::Streaming(Box::pin(stream::pending())),
        },
    );
    assert!(matches!(
        budget.try_admit_body(ProtocolOperation::Responses, 1),
        Err(PublicRequestAdmissionError::Capacity)
    ));

    drop(response);
    assert!(
        budget
            .try_admit_body(ProtocolOperation::Responses, 1)
            .is_ok()
    );
}

#[test]
fn continuation_working_set_stays_reserved_across_buffered_clones() {
    let capacity = MAX_BRIDGE_CONTINUATION_STATE_BYTES + MEMORY_UNIT_BYTES;
    let budget = PublicRequestMemoryBudget::with_capacity_bytes(capacity);
    let mut admission = budget
        .try_admit_body(ProtocolOperation::Responses, 1)
        .expect("continuation request admission");
    admission
        .reserve_execution(0, &IngressAffinity::Continuation("resp-private".into()))
        .expect("continuation working set fits exactly");

    let response = hold_response_memory(
        admission,
        PublicResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: PublicResponseBody::Buffered(Bytes::from_static(b"response")),
        },
    );
    let PublicResponseBody::Buffered(body) = response.body else {
        panic!("buffered response expected");
    };
    let clone = body.clone();
    drop(body);
    assert!(matches!(
        budget.try_admit_body(ProtocolOperation::Responses, 1),
        Err(PublicRequestAdmissionError::Capacity)
    ));

    drop(clone);
    assert!(
        budget
            .try_admit_body(ProtocolOperation::Responses, 1)
            .is_ok()
    );
}

#[test]
fn ordinary_sessions_do_not_pay_the_continuation_working_set_reservation() {
    let capacity = MEMORY_UNIT_BYTES * 5;
    let budget = PublicRequestMemoryBudget::with_capacity_bytes(capacity);
    let mut request = budget
        .try_admit_body(ProtocolOperation::Responses, 1)
        .expect("session request admission");

    request
        .reserve_execution(
            MEMORY_UNIT_BYTES,
            &IngressAffinity::Session("session-private".into()),
        )
        .expect("ordinary session execution");
    assert!(
        budget
            .try_admit_body(ProtocolOperation::Responses, 1)
            .is_ok()
    );
}

#[tokio::test]
async fn cancelled_waiter_does_not_release_a_running_blocking_admission() {
    let budget = PublicRequestMemoryBudget::with_capacity_bytes(MEMORY_UNIT_BYTES);
    let admission = budget
        .try_admit_body(ProtocolOperation::Responses, 1)
        .expect("admission");
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let waiter = tokio::spawn(admission.run_blocking(move || {
        started_tx.send(()).expect("signal blocking work");
        release_rx.recv().expect("release blocking work");
    }));
    tokio::task::spawn_blocking(move || started_rx.recv())
        .await
        .expect("join start wait")
        .expect("blocking work started");

    waiter.abort();
    assert!(matches!(waiter.await, Err(error) if error.is_cancelled()));
    assert!(matches!(
        budget.try_admit_body(ProtocolOperation::Responses, 1),
        Err(PublicRequestAdmissionError::Capacity)
    ));

    release_tx.send(()).expect("release blocking work");
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if budget
                .try_admit_body(ProtocolOperation::Responses, 1)
                .is_ok()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("blocking work must eventually release admission");
}

#[test]
fn buffered_response_bytes_hold_capacity_across_clones() {
    let budget = PublicRequestMemoryBudget::with_capacity_bytes(MEMORY_UNIT_BYTES);
    let admission = budget
        .try_admit_body(ProtocolOperation::Responses, 1)
        .expect("admission");
    let response = hold_response_memory(
        admission,
        PublicResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: PublicResponseBody::Buffered(Bytes::from_static(b"response")),
        },
    );
    let PublicResponseBody::Buffered(body) = response.body else {
        panic!("buffered response expected");
    };
    let clone = body.clone();
    drop(body);
    assert!(matches!(
        budget.try_admit_body(ProtocolOperation::Responses, 1),
        Err(PublicRequestAdmissionError::Capacity)
    ));
    drop(clone);
    assert!(
        budget
            .try_admit_body(ProtocolOperation::Responses, 1)
            .is_ok()
    );
}

#[test]
fn streaming_response_holds_capacity_until_drop() {
    let budget = PublicRequestMemoryBudget::with_capacity_bytes(MEMORY_UNIT_BYTES);
    let admission = budget
        .try_admit_body(ProtocolOperation::Responses, 1)
        .expect("admission");
    let response = hold_response_memory(
        admission,
        PublicResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: PublicResponseBody::Streaming(Box::pin(stream::pending())),
        },
    );
    assert!(matches!(
        budget.try_admit_body(ProtocolOperation::Responses, 1),
        Err(PublicRequestAdmissionError::Capacity)
    ));
    drop(response);
    assert!(
        budget
            .try_admit_body(ProtocolOperation::Responses, 1)
            .is_ok()
    );
}

#[tokio::test]
async fn streaming_response_releases_capacity_at_eof_without_waiting_for_drop() {
    let budget = PublicRequestMemoryBudget::with_capacity_bytes(MEMORY_UNIT_BYTES);
    let admission = budget
        .try_admit_body(ProtocolOperation::Responses, 1)
        .expect("admission");
    let response = hold_response_memory(
        admission,
        PublicResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: PublicResponseBody::Streaming(Box::pin(stream::empty())),
        },
    );
    let PublicResponseBody::Streaming(mut body) = response.body else {
        panic!("streaming response expected");
    };
    assert!(body.next().await.is_none());
    assert!(
        budget
            .try_admit_body(ProtocolOperation::Responses, 1)
            .is_ok()
    );
}

#[tokio::test]
async fn streaming_response_releases_capacity_at_error_without_waiting_for_drop() {
    let budget = PublicRequestMemoryBudget::with_capacity_bytes(MEMORY_UNIT_BYTES);
    let admission = budget
        .try_admit_body(ProtocolOperation::Responses, 1)
        .expect("admission");
    let response = hold_response_memory(
        admission,
        PublicResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: PublicResponseBody::Streaming(Box::pin(stream::once(async {
                Err(std::io::Error::other("stream failed"))
            }))),
        },
    );
    let PublicResponseBody::Streaming(mut body) = response.body else {
        panic!("streaming response expected");
    };
    assert!(body.next().await.expect("error item").is_err());
    assert!(
        budget
            .try_admit_body(ProtocolOperation::Responses, 1)
            .is_ok()
    );
}
