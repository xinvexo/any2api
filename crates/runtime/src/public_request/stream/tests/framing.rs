use std::{sync::Arc, time::Duration};

use any2api_domain::{PublicErrorCode, SettingsConfiguration};
use any2api_transport::api::BoxByteStream;
use bytes::Bytes;
use futures_util::{StreamExt, stream};

use super::{
    super::PrecommitBudget,
    core::{generation_permit, guarded_body_with_budget, guarded_body_with_budget_and_health},
};
use crate::{
    health::{AttemptHealth, EndpointHealthRuntime, ReliabilityPolicy},
    routing::SchedulerEpoch,
};

#[tokio::test]
async fn oversized_first_event_exhausts_the_precommit_byte_budget() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
        b"data: {\"model\":\"upstream\"}\n\n",
    ))]));
    let result = guarded_body_with_budget(
        upstream,
        permit,
        PrecommitBudget::new(16, Duration::from_secs(5)),
    )
    .prime()
    .await;

    let error = match result {
        Ok(_) => panic!("oversized first event must fail before commit"),
        Err(error) => error,
    };
    assert_eq!(error.code(), PublicErrorCode::UpstreamError);
    assert_eq!(binding.in_flight(), 0);
}

#[tokio::test]
async fn encoded_event_budget_failure_is_reported_as_upstream_error() {
    let (binding, permit) = generation_permit();
    let epoch = SchedulerEpoch::new();
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    let mut policy =
        ReliabilityPolicy::from_settings(SettingsConfiguration::defaults().reliability());
    policy.endpoint_failure_threshold = 1;
    let health = AttemptHealth::new(
        binding.generation().clone(),
        "upstream".into(),
        Some(endpoint.try_acquire(&policy).expect("endpoint permit")),
        None,
        policy,
    );
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
        b"data: {\"model\":\"u\"}\n\n",
    ))]));
    let result = guarded_body_with_budget_and_health(
        upstream,
        permit,
        PrecommitBudget::new(24, Duration::from_secs(5)),
        Some(health),
    )
    .prime()
    .await;

    let error = match result {
        Ok(_) => panic!("encoded output over budget must fail before commit"),
        Err(error) => error,
    };
    assert_eq!(error.code(), PublicErrorCode::UpstreamError);
    assert_eq!(binding.in_flight(), 0);
    assert_eq!(endpoint.availability(&policy), Ok(()));
}

#[tokio::test]
async fn complete_event_precedes_a_later_same_chunk_frame_error() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
        b"data: {\"model\":\"upstream\"}\n\ndata: this-frame-is-deliberately-longer-than-the-configured-sixty-four-byte-limit-for-this-test\n\n",
    ))]));
    let mut body = guarded_body_with_budget(
        upstream,
        permit,
        PrecommitBudget::new(64, Duration::from_secs(5)),
    )
    .prime()
    .await
    .expect("first complete event must commit")
    .into_stream();

    let first = body
        .next()
        .await
        .expect("first frame")
        .expect("first frame bytes");
    assert!(String::from_utf8_lossy(&first).contains(r#""model":"public""#));
    assert_eq!(binding.in_flight(), 1);
    assert!(body.next().await.expect("later frame error").is_err());
    assert_eq!(binding.in_flight(), 0);
    assert!(body.next().await.is_none());
}

#[tokio::test]
async fn prime_buffers_only_the_first_complete_event_from_a_chunk() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
        b"data: {\"model\":\"upstream\",\"index\":1}\n\ndata: {\"model\":\"upstream\",\"index\":2}\n\ndata: {\"model\":\"upstream\",\"index\":3}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n",
    ))]));
    let guarded = guarded_body_with_budget(
        upstream,
        permit,
        PrecommitBudget::new(256 * 1024, Duration::from_secs(5)),
    )
    .prime()
    .await
    .expect("first event");
    assert_eq!(guarded.pending_frame_count(), 1);
    let mut body = guarded.into_stream();

    for index in 1..=3 {
        let frame = body
            .next()
            .await
            .expect("stream frame")
            .expect("stream bytes");
        assert!(String::from_utf8_lossy(&frame).contains(&format!(r#""index":{index}"#)));
    }
    assert!(body.next().await.expect("terminal frame").is_ok());
    assert!(body.next().await.is_none());
    assert_eq!(binding.in_flight(), 0);
}
