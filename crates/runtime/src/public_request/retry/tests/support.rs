use std::sync::Arc;

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, ProtocolDialect, ProtocolOperation,
    ProviderBaseUrl, ProviderCredential, ProviderCredentialDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId, RetryAfterHint, RetrySafety, RouteTargetId, RoutingCredentialId,
    SettingsConfiguration, UpstreamError, UpstreamErrorClassification, UpstreamErrorKind,
    UpstreamFailureAttribution,
};
use any2api_protocol::api::RequestExecutionProfile;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

use crate::{
    credential::{CredentialAuthMaterial, CredentialRuntimeHandle},
    health::EndpointHealthRuntime,
    public_request::{retry::budget::RetryBudget, upstream::AttemptFailure},
    routing::{RouteCandidate, SchedulerEpoch},
};

pub(super) fn upstream_failure(
    candidate: RouteCandidate,
    bound: bool,
    kind: UpstreamErrorKind,
    safety: RetrySafety,
    attribution: UpstreamFailureAttribution,
) -> AttemptFailure {
    upstream_failure_with_retry_after(candidate, bound, kind, safety, attribution, None)
}

pub(super) fn upstream_failure_with_retry_after(
    candidate: RouteCandidate,
    bound: bool,
    kind: UpstreamErrorKind,
    safety: RetrySafety,
    attribution: UpstreamFailureAttribution,
    retry_after: Option<RetryAfterHint>,
) -> AttemptFailure {
    AttemptFailure::Upstream {
        status: StatusCode::UNAUTHORIZED,
        headers: Box::new(HeaderMap::new()),
        body: Bytes::new(),
        error: Box::new(UpstreamError::new(
            UpstreamErrorClassification::new(kind, safety, retry_after)
                .with_attribution(attribution),
            None,
        )),
        candidate: Box::new(candidate),
        bound,
    }
}

pub(super) fn attempted_budget(credential_id: RoutingCredentialId) -> RetryBudget {
    let mut policy = crate::health::ReliabilityPolicy::from_settings(
        SettingsConfiguration::defaults().reliability(),
    );
    policy.base_delay = std::time::Duration::from_secs(1);
    policy.jitter_ratio = 0;
    let mut budget = RetryBudget::new(
        policy,
        ProtocolOperation::Responses,
        RequestExecutionProfile::Standard,
    );
    assert_eq!(budget.register_attempt(credential_id), Some(1));
    budget
}

pub(super) fn candidate(label: &str) -> RouteCandidate {
    let scheduler_epoch = SchedulerEpoch::new();
    let credential = ProviderCredential::create(
        CredentialId::new(),
        ProviderEndpointId::new(),
        ProviderCredentialDraft::new(
            label,
            CredentialKind::ApiKey,
            ProxyProfileId::DIRECT,
            None,
            true,
        )
        .expect("credential draft"),
        CredentialSecretFingerprint::new([7; 32], None).expect("fingerprint"),
    );
    let binding = CredentialRuntimeHandle::new_for_provider_test(
        &credential,
        CredentialAuthMaterial::for_test(&credential, format!("sk-{label}")),
        Arc::clone(&scheduler_epoch),
    );
    RouteCandidate {
        target_id: RouteTargetId::new(),
        operation: ProtocolOperation::Responses,
        endpoint_id: credential.provider_endpoint_id(),
        endpoint_config_version: 1,
        credential_id: credential.id().into(),
        routing_generation: binding.generation().routing_generation(),
        provider_kind: ProviderKind::Codex,
        base_url: ProviderBaseUrl::parse("https://api.example.com").expect("base URL"),
        upstream_model: "gpt-test".into(),
        upstream_protocol_dialect: ProtocolDialect::OpenAiResponses,
        proxy_id: ProxyProfileId::DIRECT,
        proxy_config_version: 1,
        endpoint_health: None,
        proxy_health: None,
        egress_path_health: EndpointHealthRuntime::new(Arc::clone(&scheduler_epoch)),
        candidate_path_health: EndpointHealthRuntime::new(scheduler_epoch),
        binding,
    }
}
