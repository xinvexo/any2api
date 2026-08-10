use std::{sync::Arc, time::Duration};

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, ModelRouteId, ProtocolOperation,
    ProviderCredential, ProviderCredentialDraft, ProviderEndpointId, ProxyProfileId,
    PublicErrorCode, RequestsPerMinute, RetrySafety, RouteTargetId, SettingsConfiguration,
};
use any2api_protocol::{OpenAiResponsesAdapter, ProtocolRegistry, api::ProtocolAdapter};
use any2api_transport::api::{
    BoxByteStream, TransportError, TransportErrorStage, TransportFailureScope,
};
use bytes::Bytes;
use futures_util::{StreamExt, stream};

use super::super::{CommitState, GuardedBody, GuardedBodyParts, PrecommitBudget};
use crate::{
    affinity::{AffinityRegistry, AffinityTarget, ContinuationBindingCommitter},
    credential::{CredentialAuthMaterial, CredentialRuntimeHandle, RoutingPermit},
    health::{AttemptHealth, EndpointHealthRuntime, ReliabilityPolicy},
    request_telemetry::AttemptRecorder,
    routing::SchedulerEpoch,
};

#[tokio::test]
async fn guarded_body_primes_rewrites_and_releases_on_terminal_event() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([
        Ok(Bytes::from_static(
            b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_stream\",\"model\":\"upstream\"}}\n\n",
        )),
        Ok(Bytes::from_static(
            b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n",
        )),
    ]));
    let mut body = guarded_body(upstream, permit)
        .prime()
        .await
        .expect("primed stream")
        .into_stream();

    assert_eq!(binding.in_flight(), 1);
    let first = body
        .next()
        .await
        .expect("first frame")
        .expect("first bytes");
    assert!(String::from_utf8_lossy(&first).contains(r#""model":"public""#));
    assert_eq!(binding.in_flight(), 1);
    assert!(body.next().await.expect("terminal frame").is_ok());
    assert!(body.next().await.is_none());
    assert_eq!(binding.in_flight(), 0);
    assert_eq!(binding.rate_snapshot().requests_in_window(), 1);
}

#[tokio::test]
async fn stream_timing_marks_frame_commit_and_first_downstream_yield_in_order() {
    let (_binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([
        Ok(Bytes::from_static(
            b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_timing\"}}\n\n",
        )),
        Ok(Bytes::from_static(
            b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n",
        )),
    ]));
    let mut body = guarded_body(upstream, permit)
        .prime()
        .await
        .expect("primed stream");

    let primed = body.stream_timing();
    assert!(primed.first_upstream_frame_ms.is_some());
    assert!(primed.stream_commit_ms.is_some());
    assert!(primed.first_downstream_byte_ms.is_none());
    assert!(primed.stream_cancel_ms.is_none());

    assert!(body.next().await.expect("first frame").is_ok());
    let yielded = body.stream_timing();
    assert!(yielded.first_downstream_byte_ms.is_some());
    assert!(yielded.stream_cancel_ms.is_none());
}

#[tokio::test]
async fn dropping_body_releases_once_and_marks_cancellation() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::pending());
    let guarded = guarded_body(upstream, permit);
    let cancellation = guarded.cancellation();
    assert_eq!(guarded.state(), CommitState::Pending);
    assert_eq!(binding.in_flight(), 1);

    drop(guarded);

    assert!(cancellation.is_cancelled());
    assert_eq!(binding.in_flight(), 0);
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

    assert_eq!(error.code(), PublicErrorCode::UpstreamError);
    assert_eq!(binding.in_flight(), 0);
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
    assert_eq!(error.code(), PublicErrorCode::UpstreamError);
    assert_eq!(binding.in_flight(), 0);
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
    assert!(body.next().await.expect("terminal frame").is_ok());
    assert!(body.next().await.is_none());
    assert_eq!(binding.in_flight(), 0);
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
    assert_eq!(binding.in_flight(), 1);
    assert!(body.next().await.expect("stream error").is_err());
    assert_eq!(binding.in_flight(), 0);
    drop(body);
    assert_eq!(binding.in_flight(), 0);
}

pub(super) fn guarded_body(upstream: BoxByteStream, permit: RoutingPermit) -> GuardedBody {
    guarded_body_with_budget(
        upstream,
        permit,
        PrecommitBudget::new(256 * 1024, Duration::from_secs(5)),
    )
}

pub(super) fn guarded_body_with_budget(
    upstream: BoxByteStream,
    permit: RoutingPermit,
    precommit_budget: PrecommitBudget,
) -> GuardedBody {
    guarded_body_with_budget_and_health(upstream, permit, precommit_budget, None)
}

fn guarded_body_with_budget_and_health(
    upstream: BoxByteStream,
    permit: RoutingPermit,
    precommit_budget: PrecommitBudget,
    health: Option<AttemptHealth>,
) -> GuardedBody {
    guarded_body_with_budget_health_and_idle(
        upstream,
        permit,
        precommit_budget,
        health,
        Duration::from_secs(60),
    )
}

pub(super) fn guarded_body_with_budget_health_and_idle(
    upstream: BoxByteStream,
    permit: RoutingPermit,
    precommit_budget: PrecommitBudget,
    health: Option<AttemptHealth>,
    postcommit_idle_timeout: Duration,
) -> GuardedBody {
    guarded_body_with_adapter(
        upstream,
        permit,
        precommit_budget,
        health,
        postcommit_idle_timeout,
        Arc::new(OpenAiResponsesAdapter::new()),
        ProtocolOperation::Responses,
    )
}

pub(super) fn guarded_body_for_adapter(
    upstream: BoxByteStream,
    permit: RoutingPermit,
    adapter: Arc<dyn ProtocolAdapter>,
    operation: ProtocolOperation,
) -> GuardedBody {
    guarded_body_with_adapter(
        upstream,
        permit,
        PrecommitBudget::new(256 * 1024, Duration::from_secs(5)),
        None,
        Duration::from_secs(60),
        adapter,
        operation,
    )
}

pub(super) fn guarded_body_for_adapter_with_health(
    upstream: BoxByteStream,
    permit: RoutingPermit,
    adapter: Arc<dyn ProtocolAdapter>,
    operation: ProtocolOperation,
    health: AttemptHealth,
) -> GuardedBody {
    guarded_body_with_adapter(
        upstream,
        permit,
        PrecommitBudget::new(256 * 1024, Duration::from_secs(5)),
        Some(health),
        Duration::from_secs(60),
        adapter,
        operation,
    )
}

fn guarded_body_with_adapter(
    upstream: BoxByteStream,
    permit: RoutingPermit,
    precommit_budget: PrecommitBudget,
    health: Option<AttemptHealth>,
    postcommit_idle_timeout: Duration,
    adapter: Arc<dyn ProtocolAdapter>,
    operation: ProtocolOperation,
) -> GuardedBody {
    let dialect = adapter.dialect();
    let target = AffinityTarget::new(
        ModelRouteId::new(),
        RouteTargetId::new(),
        permit.credential_id(),
        "upstream",
        dialect,
        dialect,
    );
    let continuation_binding = ContinuationBindingCommitter::new(
        operation,
        AffinityRegistry::new(),
        target,
        Duration::from_secs(60),
    );
    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(adapter)
        .expect("streaming protocol adapter");
    let exchange = protocols
        .exchange(dialect, dialect, operation)
        .expect("direct protocol exchange");
    GuardedBody::new(
        upstream,
        exchange,
        "public",
        GuardedBodyParts {
            permit,
            health,
            continuation_binding,
            attempt_recorder: AttemptRecorder::disabled(),
            quota_activity: None,
            status_code: 200,
            precommit_budget,
            postcommit_idle_timeout,
        },
    )
}

pub(super) fn generation_permit() -> (crate::credential::CredentialRuntimeBinding, RoutingPermit) {
    let credential = ProviderCredential::create(
        CredentialId::new(),
        ProviderEndpointId::new(),
        ProviderCredentialDraft::new(
            "stream",
            CredentialKind::ApiKey,
            ProxyProfileId::DIRECT,
            Some(RequestsPerMinute::new(1).expect("valid RPM")),
            true,
        )
        .expect("credential draft"),
        CredentialSecretFingerprint::new([9; 32], None).expect("fingerprint"),
    );
    let binding = CredentialRuntimeHandle::new_for_provider_test(
        &credential,
        CredentialAuthMaterial::for_test(&credential, "sk-stream-test".into()),
        SchedulerEpoch::new(),
    );
    let permit = binding.try_reserve().expect("generation permit");
    (binding, permit)
}
