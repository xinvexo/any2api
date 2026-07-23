use std::sync::Arc;

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, MaxConcurrency, ProtocolDialect,
    ProtocolOperation, ProviderCredential, ProviderCredentialDraft, ProviderEndpointId,
    ProxyProfile, ProxyProfileId, PublicErrorCode,
};
use any2api_protocol::{OpenAiResponsesAdapter, ProtocolRegistry};
use any2api_provider::{CodexDriver, api::ProviderDriver};
use any2api_transport::api::TransportProxy;

use super::PreparedAttempt;
use crate::{
    credential_auth::CredentialAuthMaterial,
    credential_runtime::CredentialRuntimeHandle,
    health::{AttemptHealth, EndpointHealthRuntime, ReliabilityPolicy},
    public_request::{RequestPermit, response::public_error},
    request_telemetry::AttemptRecorder,
    scheduler_epoch::SchedulerEpoch,
};

#[tokio::test(start_paused = true)]
async fn postprocess_failure_closes_half_open_health_before_releasing_capacity() {
    let epoch = SchedulerEpoch::new();
    let policy = ReliabilityPolicy::from_settings(
        any2api_domain::SettingsConfiguration::defaults().reliability(),
    );
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    let endpoint_permits = (0..policy.endpoint_failure_threshold)
        .map(|_| endpoint.try_acquire(&policy).expect("closed endpoint"))
        .collect::<Vec<_>>();
    for permit in endpoint_permits {
        permit.failure(&policy);
    }
    tokio::time::advance(policy.endpoint_open_duration).await;

    let credential = ProviderCredential::create(
        CredentialId::new(),
        ProviderEndpointId::new(),
        ProviderCredentialDraft::new(
            "postprocess",
            CredentialKind::ApiKey,
            ProxyProfileId::DIRECT,
            MaxConcurrency::new(1).expect("max concurrency"),
            true,
        )
        .expect("credential draft"),
        CredentialSecretFingerprint::new([7; 32], None).expect("fingerprint"),
    );
    let binding = CredentialRuntimeHandle::new(
        &credential,
        CredentialAuthMaterial::for_test(&credential, "sk-postprocess-test".into()),
        epoch,
    )
    .current_binding();
    let permit = binding.try_acquire().expect("credential permit");
    let health = AttemptHealth::new(
        Arc::clone(binding.generation()),
        "upstream-model".into(),
        Some(endpoint.try_acquire(&policy).expect("half-open probe")),
        None,
        policy,
    );
    let driver = CodexDriver::new();
    let proxy = ProxyProfile::direct();
    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("Responses adapter");
    let exchange = protocols
        .exchange(
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiResponses,
            ProtocolOperation::Responses,
        )
        .expect("direct Responses exchange");
    let mut prepared = PreparedAttempt {
        driver: &driver as &dyn ProviderDriver,
        proxy: TransportProxy::new(&proxy, None),
        ingress_operation: ProtocolOperation::Responses,
        upstream_operation: ProtocolOperation::Responses,
        exchange: Some(exchange),
        request: None,
        permit: Some(RequestPermit::Generation(permit)),
        health: Some(health),
        attempt_recorder: Some(AttemptRecorder::disabled()),
    };

    let failure = prepared.fail_after_upstream_success(
        200,
        public_error(PublicErrorCode::InternalError, "test postprocess failure"),
    );

    assert!(matches!(failure, super::AttemptFailure::Public(_)));
    assert_eq!(binding.capacity().in_flight(), 0);
    let first = endpoint
        .try_acquire(&policy)
        .expect("closed endpoint first permit");
    let second = endpoint
        .try_acquire(&policy)
        .expect("closed endpoint second permit");
    drop(first);
    drop(second);
}
