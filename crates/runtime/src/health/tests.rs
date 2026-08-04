use std::{sync::Arc, time::Duration};

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, ProviderCredential,
    ProviderCredentialDraft, ProviderEndpointId, ProxyProfileId, RetryAfterHint, RetrySafety,
    SettingsConfiguration, UpstreamErrorClassification, UpstreamErrorKind, UpstreamQuotaExhaustion,
};
use any2api_transport::api::TransportFailureScope;

use super::{
    AttemptHealth, HealthAcquireError, ReliabilityPolicy,
    circuit::CircuitRuntime,
    runtime::{CredentialHealthRuntime, EndpointHealthRuntime, ProxyHealthRuntime},
};
use crate::{
    credential::{CredentialAuthMaterial, CredentialGenerationRuntime, CredentialRuntimeHandle},
    lifecycle::ProcessLifecycle,
    routing::SchedulerEpoch,
};

#[tokio::test(start_paused = true)]
async fn rate_limit_cools_only_the_model_and_wakes_at_expiry() {
    let epoch = SchedulerEpoch::new();
    let health = CredentialHealthRuntime::new(Arc::clone(&epoch));
    let policy = policy();
    health.record(
        "model-a",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::RateLimited,
            RetrySafety::RejectedBeforeExecution,
            Some(RetryAfterHint::Delay(Duration::from_secs(2))),
        ),
        &policy,
    );

    assert!(matches!(
        health.availability("model-a"),
        Err(HealthAcquireError::Temporary(_))
    ));
    assert_eq!(health.availability("model-b"), Ok(()));
    tokio::time::advance(Duration::from_secs(2)).await;
    tokio::task::yield_now().await;
    assert_eq!(health.availability("model-a"), Ok(()));
    assert!(epoch.current() > 0);
}

#[tokio::test(start_paused = true)]
async fn expired_model_cooldowns_are_lazily_reclaimed_on_health_access() {
    let epoch = SchedulerEpoch::new();
    let health = CredentialHealthRuntime::new(Arc::clone(&epoch));
    let policy = policy();
    let rate_limit = UpstreamErrorClassification::new(
        UpstreamErrorKind::RateLimited,
        RetrySafety::RejectedBeforeExecution,
        Some(RetryAfterHint::Delay(Duration::from_secs(2))),
    );

    health.record("model-a", rate_limit, &policy);
    health.record("model-b", rate_limit, &policy);
    assert_eq!(health.model_cooldown_count(), 2);

    tokio::time::advance(Duration::from_secs(2)).await;
    assert_eq!(health.availability("model-a"), Ok(()));
    assert_eq!(health.model_cooldown_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn unbounded_retry_after_is_clamped_without_becoming_immediately_available() {
    let epoch = SchedulerEpoch::new();
    let health = CredentialHealthRuntime::new(epoch);
    let policy = policy();
    let started_at = tokio::time::Instant::now();
    health.record(
        "model",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::RateLimited,
            RetrySafety::RejectedBeforeExecution,
            Some(RetryAfterHint::Delay(Duration::from_secs(u64::MAX))),
        ),
        &policy,
    );

    let until = match health.availability("model") {
        Err(HealthAcquireError::Temporary(unavailability)) => unavailability.until(),
        other => panic!("expected bounded cooldown, got {other:?}"),
    };
    assert_eq!(
        until.duration_since(started_at),
        Duration::from_secs(any2api_domain::MAX_RETRY_AFTER_SECONDS)
    );
    tokio::time::advance(Duration::from_secs(24 * 60 * 60)).await;
    assert!(matches!(
        health.availability("model"),
        Err(HealthAcquireError::Temporary(_))
    ));
}

#[test]
fn authentication_error_is_generation_local_and_permanent() {
    let epoch = SchedulerEpoch::new();
    let health = CredentialHealthRuntime::new(Arc::clone(&epoch));
    health.record(
        "model",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::Authentication,
            RetrySafety::RejectedBeforeExecution,
            None,
        ),
        &policy(),
    );
    assert_eq!(
        health.availability("model"),
        Err(HealthAcquireError::Permanent)
    );
    let marked_epoch = epoch.current();
    assert!(health.has_auth_error());
    assert!(health.clear_auth_error());
    assert!(!health.has_auth_error());
    assert_eq!(health.availability("model"), Ok(()));
    assert!(epoch.current() > marked_epoch);
    assert!(!health.clear_auth_error());
    let replacement_generation = CredentialHealthRuntime::new(epoch);
    assert_eq!(replacement_generation.availability("model"), Ok(()));
}

#[tokio::test]
async fn quota_reset_clears_temporary_cooldowns_but_keeps_authentication_failure() {
    let epoch = SchedulerEpoch::new();
    let health = CredentialHealthRuntime::new(Arc::clone(&epoch));
    let policy = policy();
    health.record(
        "model-a",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::QuotaExhausted,
            RetrySafety::RejectedBeforeExecution,
            None,
        ),
        &policy,
    );
    health.record(
        "model-b",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::RateLimited,
            RetrySafety::RejectedBeforeExecution,
            None,
        ),
        &policy,
    );
    health.record(
        "model-a",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::Authentication,
            RetrySafety::RejectedBeforeExecution,
            None,
        ),
        &policy,
    );
    let before_reset = epoch.current();

    assert!(health.clear_temporary_cooldowns());
    assert!(epoch.current() > before_reset);
    assert_eq!(
        health.availability("model-a"),
        Err(HealthAcquireError::Permanent)
    );
    assert!(health.has_auth_error());

    assert!(health.clear_auth_error());
    assert_eq!(health.availability("model-a"), Ok(()));
    assert_eq!(health.availability("model-b"), Ok(()));
}

#[tokio::test]
async fn real_quota_exhaustion_observation_is_generation_local_and_success_clears_it() {
    let epoch = SchedulerEpoch::new();
    let health = CredentialHealthRuntime::new(epoch);
    health.record(
        "grok-4.5",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::QuotaExhausted,
            RetrySafety::RejectedBeforeExecution,
            None,
        )
        .with_quota_exhaustion(UpstreamQuotaExhaustion::new(1_065_387, 1_000_000)),
        &policy(),
    );

    let observed = health.quota_exhaustion().expect("quota observation");
    assert_eq!(observed.used, Some(1_065_387));
    assert_eq!(observed.limit, Some(1_000_000));
    assert!(observed.observed_at > 0);
    assert!(matches!(
        health.availability("grok-4.5"),
        Err(HealthAcquireError::Temporary(_))
    ));

    health.record_success();
    assert!(health.quota_exhaustion().is_none());
    assert_eq!(health.availability("grok-4.5"), Ok(()));
}

#[tokio::test(start_paused = true)]
async fn quota_exhaustion_wakes_and_allows_a_probe_at_reset_time() {
    let epoch = SchedulerEpoch::new();
    let health = CredentialHealthRuntime::new(Arc::clone(&epoch));
    let before = epoch.current();

    health.record_quota_exhaustion(Duration::from_secs(2), None, None);
    assert!(epoch.current() > before);
    assert!(matches!(
        health.availability("model"),
        Err(HealthAcquireError::Temporary(_))
    ));
    let marked = epoch.current();

    tokio::time::advance(Duration::from_secs(2)).await;
    tokio::task::yield_now().await;
    assert_eq!(health.availability("model"), Ok(()));
    assert!(epoch.current() > marked);
}

#[tokio::test(start_paused = true)]
async fn repeated_health_failures_share_one_background_wake_worker() {
    let lifecycle = ProcessLifecycle::new();
    let epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
    let credential = CredentialHealthRuntime::new(Arc::clone(&epoch));
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    let proxy = ProxyHealthRuntime::new(Arc::clone(&epoch));
    let policy = policy();
    let rate_limit = UpstreamErrorClassification::new(
        UpstreamErrorKind::RateLimited,
        RetrySafety::RejectedBeforeExecution,
        Some(RetryAfterHint::Delay(Duration::from_secs(60))),
    );

    for model in 0..128 {
        let model = format!("model-{model}");
        credential.record(&model, rate_limit, &policy);
        credential.record(&model, rate_limit, &policy);
    }
    let endpoint_permits = (0..policy.endpoint_failure_threshold)
        .map(|_| endpoint.try_acquire(&policy).expect("closed endpoint"))
        .collect::<Vec<_>>();
    for permit in endpoint_permits {
        permit.failure(&policy);
    }
    let proxy_permits = (0..policy.proxy_failure_threshold)
        .map(|_| proxy.try_acquire(&policy).expect("closed proxy"))
        .collect::<Vec<_>>();
    for permit in proxy_permits {
        permit.failure(&policy);
    }

    assert_eq!(lifecycle.background_task_count(), 1);
    lifecycle.begin_draining();
    lifecycle.close_background_tasks();
    lifecycle.wait_for_background_tasks().await;
    assert_eq!(lifecycle.background_task_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn endpoint_and_proxy_breakers_open_and_allow_a_single_half_open_probe() {
    let epoch = SchedulerEpoch::new();
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    let proxy = ProxyHealthRuntime::new(epoch);
    let policy = policy();

    let endpoint_permits = (0..policy.endpoint_failure_threshold)
        .map(|_| endpoint.try_acquire(&policy).expect("closed endpoint"))
        .collect::<Vec<_>>();
    for permit in endpoint_permits {
        permit.failure(&policy);
    }
    assert!(matches!(
        endpoint.availability(&policy),
        Err(HealthAcquireError::Temporary(_))
    ));
    tokio::time::advance(policy.endpoint_open_duration).await;
    let endpoint_probe = endpoint.try_acquire(&policy).expect("half-open probe");
    assert!(endpoint.try_acquire(&policy).is_err());
    endpoint_probe.success(tokio::time::Instant::now());
    assert_eq!(endpoint.availability(&policy), Ok(()));

    for _ in 0..policy.proxy_failure_threshold {
        proxy
            .try_acquire(&policy)
            .expect("closed proxy")
            .failure(&policy);
    }
    assert!(proxy.availability(&policy).is_err());
    tokio::time::advance(policy.proxy_open_duration).await;
    let proxy_probe = proxy.try_acquire(&policy).expect("proxy probe");
    assert!(proxy.try_acquire(&policy).is_err());
    proxy_probe.success();
    assert_eq!(proxy.availability(&policy), Ok(()));
}

#[test]
fn unattributed_transport_failure_does_not_pollute_endpoint_or_proxy_health() {
    let epoch = SchedulerEpoch::new();
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    let proxy = ProxyHealthRuntime::new(Arc::clone(&epoch));
    let mut policy = policy();
    policy.endpoint_failure_threshold = 1;
    policy.proxy_failure_threshold = 1;
    let generation = test_generation(Arc::clone(&epoch));
    let health = AttemptHealth::new(
        Arc::clone(&generation),
        "model".into(),
        Some(endpoint.try_acquire(&policy).expect("endpoint permit")),
        Some(proxy.try_acquire(&policy).expect("proxy permit")),
        policy,
    );

    health.transport_failure(TransportFailureScope::Unattributed);

    assert_eq!(endpoint.availability(&policy), Ok(()));
    assert_eq!(proxy.availability(&policy), Ok(()));
}

#[tokio::test(start_paused = true)]
async fn proxy_transport_failure_opens_only_proxy_health() {
    let epoch = SchedulerEpoch::new();
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    let proxy = ProxyHealthRuntime::new(Arc::clone(&epoch));
    let mut policy = policy();
    policy.endpoint_failure_threshold = 1;
    policy.proxy_failure_threshold = 1;
    let generation = test_generation(Arc::clone(&epoch));
    let health = AttemptHealth::new(
        Arc::clone(&generation),
        "model".into(),
        Some(endpoint.try_acquire(&policy).expect("endpoint permit")),
        Some(proxy.try_acquire(&policy).expect("proxy permit")),
        policy,
    );

    health.transport_failure(TransportFailureScope::Proxy);

    assert_eq!(endpoint.availability(&policy), Ok(()));
    assert!(proxy.availability(&policy).is_err());
    assert_eq!(generation.health().availability("model"), Ok(()));
}

#[tokio::test(start_paused = true)]
async fn circuit_failure_window_uses_recent_failures() {
    let circuit = CircuitRuntime::new(SchedulerEpoch::new());
    let failure_window = Duration::from_secs(30);
    let open_duration = Duration::from_secs(10);

    assert_eq!(
        circuit
            .try_acquire(1)
            .expect("closed circuit")
            .failure(3, failure_window, open_duration),
        None
    );
    tokio::time::advance(Duration::from_secs(20)).await;
    assert_eq!(
        circuit
            .try_acquire(1)
            .expect("closed circuit")
            .failure(3, failure_window, open_duration),
        None
    );
    tokio::time::advance(Duration::from_secs(11)).await;
    assert_eq!(
        circuit
            .try_acquire(1)
            .expect("closed circuit")
            .failure(3, failure_window, open_duration),
        None
    );
    tokio::time::advance(Duration::from_secs(9)).await;
    assert!(
        circuit
            .try_acquire(1)
            .expect("closed circuit")
            .failure(3, failure_window, open_duration)
            .is_some()
    );
    assert!(circuit.availability(1).is_err());
}

fn policy() -> ReliabilityPolicy {
    ReliabilityPolicy::from_settings(SettingsConfiguration::defaults().reliability())
}

fn test_generation(epoch: Arc<SchedulerEpoch>) -> Arc<CredentialGenerationRuntime> {
    let credential = ProviderCredential::create(
        CredentialId::new(),
        ProviderEndpointId::new(),
        ProviderCredentialDraft::new(
            "health",
            CredentialKind::ApiKey,
            ProxyProfileId::DIRECT,
            None,
            true,
        )
        .expect("credential draft"),
        CredentialSecretFingerprint::new([8; 32], None).expect("fingerprint"),
    );
    CredentialRuntimeHandle::new_for_provider_test(
        &credential,
        CredentialAuthMaterial::for_test(&credential, "sk-health-test".into()),
        epoch,
    )
    .generation()
    .clone()
}
