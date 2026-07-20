use std::{sync::Arc, time::Duration};

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, MaxConcurrency, ModelRouteId,
    ProtocolDialect, ProtocolOperation, ProviderCredential, ProviderCredentialDraft,
    ProviderEndpointId, ProxyProfileId, PublicErrorCode, RetrySafety, RouteTargetId,
    SettingsConfiguration,
};
use any2api_protocol::OpenAiResponsesAdapter;
use any2api_transport::api::{
    BoxByteStream, TransportError, TransportErrorStage, TransportFailureScope,
};
use bytes::Bytes;
use futures_util::{StreamExt, stream};

use super::{
    RequestPermit,
    stream::{CommitState, GuardedBody, GuardedBodyParts, PrecommitBudget},
};
use crate::{
    affinity::{AffinityRegistry, AffinityTarget, HardAffinityCommitter},
    credential_auth::CredentialAuthMaterial,
    credential_runtime::CredentialRuntimeHandle,
    health::{AttemptHealth, EndpointHealthRuntime, ReliabilityPolicy},
    request_telemetry::AttemptRecorder,
    scheduler_epoch::SchedulerEpoch,
};

#[tokio::test]
async fn guarded_body_primes_rewrites_and_releases_on_eof() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([
        Ok(Bytes::from_static(
            b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_stream\",\"model\":\"upstream\"}}\n\n",
        )),
        Ok(Bytes::from_static(b"data: [DONE]\n\n")),
    ]));
    let mut body = guarded_body(upstream, permit)
        .prime()
        .await
        .expect("primed stream")
        .into_stream();

    assert_eq!(binding.capacity().in_flight(), 1);
    let first = body
        .next()
        .await
        .expect("first frame")
        .expect("first bytes");
    assert!(String::from_utf8_lossy(&first).contains(r#""model":"public""#));
    assert_eq!(binding.capacity().in_flight(), 1);
    assert!(body.next().await.expect("done frame").is_ok());
    assert!(body.next().await.is_none());
    assert_eq!(binding.capacity().in_flight(), 0);
}

#[tokio::test]
async fn dropping_body_releases_once_and_marks_cancellation() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::pending());
    let guarded = guarded_body(upstream, permit);
    let cancellation = guarded.cancellation();
    assert_eq!(guarded.state(), CommitState::Pending);
    assert_eq!(binding.capacity().in_flight(), 1);

    drop(guarded);

    assert!(cancellation.is_cancelled());
    assert_eq!(binding.capacity().in_flight(), 0);
}

#[tokio::test]
async fn empty_stream_fails_before_commit_and_releases() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::empty());
    let result = guarded_body(upstream, permit).prime().await;
    let error = match result {
        Ok(_) => panic!("empty stream must fail before commit"),
        Err(error) => error,
    };

    assert_eq!(error.code, PublicErrorCode::UpstreamError);
    assert_eq!(binding.capacity().in_flight(), 0);
}

#[tokio::test]
async fn transport_error_before_the_first_frame_releases_without_commit() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([Err(TransportError::new(
        TransportErrorStage::ReadBody,
        TransportFailureScope::Endpoint,
        RetrySafety::Ambiguous,
        "test precommit failure",
    ))]));
    let result = guarded_body(upstream, permit).prime().await;

    let error = match result {
        Ok(_) => panic!("precommit transport error must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code, PublicErrorCode::UpstreamError);
    assert_eq!(binding.capacity().in_flight(), 0);
}

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
    assert_eq!(error.code, PublicErrorCode::UpstreamError);
    assert_eq!(binding.capacity().in_flight(), 0);
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
    assert_eq!(error.code, PublicErrorCode::UpstreamError);
    assert_eq!(binding.capacity().in_flight(), 0);
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
    assert_eq!(binding.capacity().in_flight(), 1);
    assert!(body.next().await.expect("later frame error").is_err());
    assert_eq!(binding.capacity().in_flight(), 0);
    assert!(body.next().await.is_none());
}

#[tokio::test]
async fn prime_buffers_only_the_first_complete_event_from_a_chunk() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
        b"data: {\"model\":\"upstream\",\"index\":1}\n\ndata: {\"model\":\"upstream\",\"index\":2}\n\ndata: {\"model\":\"upstream\",\"index\":3}\n\n",
    ))]));
    let guarded = guarded_body(upstream, permit)
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
    assert!(body.next().await.is_none());
    assert_eq!(binding.capacity().in_flight(), 0);
}

#[tokio::test(start_paused = true)]
async fn configured_precommit_duration_bounds_the_first_event_wait() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::pending());
    let result = guarded_body_with_budget(
        upstream,
        permit,
        PrecommitBudget::new(256 * 1024, Duration::from_millis(25)),
    )
    .prime()
    .await;

    let error = match result {
        Ok(_) => panic!("precommit wait must be bounded"),
        Err(error) => error,
    };
    assert_eq!(error.code, PublicErrorCode::UpstreamError);
    assert_eq!(binding.capacity().in_flight(), 0);
}

#[tokio::test]
async fn precommit_deadline_is_checked_after_a_non_cooperative_upstream_poll() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::once(async {
        std::thread::sleep(Duration::from_millis(20));
        Ok(Bytes::from_static(b"data: {\"model\":\"upstream\"}\n\n"))
    }));
    let result = guarded_body_with_budget(
        upstream,
        permit,
        PrecommitBudget::new(256 * 1024, Duration::from_millis(1)),
    )
    .prime()
    .await;

    let error = match result {
        Ok(_) => panic!("event completed after the deadline must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code, PublicErrorCode::UpstreamError);
    assert_eq!(binding.capacity().in_flight(), 0);
}

#[tokio::test]
async fn post_commit_error_releases_without_emitting_another_upstream() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([
        Ok(Bytes::from_static(b"data: {\"model\":\"upstream\"}\n\n")),
        Err(TransportError::new(
            TransportErrorStage::ReadBody,
            TransportFailureScope::Endpoint,
            RetrySafety::Ambiguous,
            "test body failure",
        )),
    ]));
    let mut body = guarded_body(upstream, permit)
        .prime()
        .await
        .expect("primed stream")
        .into_stream();

    assert!(body.next().await.expect("first frame").is_ok());
    assert_eq!(binding.capacity().in_flight(), 1);
    assert!(body.next().await.expect("stream error").is_err());
    assert_eq!(binding.capacity().in_flight(), 0);
    drop(body);
    assert_eq!(binding.capacity().in_flight(), 0);
}

fn guarded_body(
    upstream: BoxByteStream,
    permit: crate::credential_runtime::ConcurrencyPermit,
) -> GuardedBody {
    guarded_body_with_budget(
        upstream,
        permit,
        PrecommitBudget::new(256 * 1024, Duration::from_secs(5)),
    )
}

fn guarded_body_with_budget(
    upstream: BoxByteStream,
    permit: crate::credential_runtime::ConcurrencyPermit,
    precommit_budget: PrecommitBudget,
) -> GuardedBody {
    guarded_body_with_budget_and_health(upstream, permit, precommit_budget, None)
}

fn guarded_body_with_budget_and_health(
    upstream: BoxByteStream,
    permit: crate::credential_runtime::ConcurrencyPermit,
    precommit_budget: PrecommitBudget,
    health: Option<AttemptHealth>,
) -> GuardedBody {
    let target = AffinityTarget::new(
        ModelRouteId::new(),
        RouteTargetId::new(),
        permit.credential_id(),
        "upstream",
        ProtocolDialect::OpenAiResponses,
    );
    let hard_affinity = HardAffinityCommitter::new(
        ProtocolOperation::Responses,
        AffinityRegistry::new(),
        target,
        Duration::from_secs(60),
    );
    GuardedBody::new(
        upstream,
        Arc::new(OpenAiResponsesAdapter::new()),
        "public",
        GuardedBodyParts {
            permit: RequestPermit::Generation(permit),
            health,
            hard_affinity,
            attempt_recorder: AttemptRecorder::disabled(),
            status_code: 200,
            precommit_budget,
        },
    )
}

fn generation_permit() -> (
    crate::credential_runtime::CredentialRuntimeBinding,
    crate::credential_runtime::ConcurrencyPermit,
) {
    let credential = ProviderCredential::create(
        CredentialId::new(),
        ProviderEndpointId::new(),
        ProviderCredentialDraft::new(
            "stream",
            CredentialKind::ApiKey,
            ProxyProfileId::DIRECT,
            MaxConcurrency::new(1).expect("max concurrency"),
            true,
        )
        .expect("credential draft"),
        CredentialSecretFingerprint::new([9; 32], None).expect("fingerprint"),
    );
    let binding = CredentialRuntimeHandle::new(
        &credential,
        CredentialAuthMaterial::for_test(&credential, "sk-stream-test".into()),
        SchedulerEpoch::new(),
    )
    .current_binding();
    let permit = binding.try_acquire().expect("generation permit");
    (binding, permit)
}
