use std::{sync::Arc, time::Duration};

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, OAuthAccountId, ProtocolDialect,
    ProtocolOperation, ProviderBaseUrl, ProviderCredential, ProviderCredentialDraft,
    ProviderEndpointId, ProviderKind, ProxyProfileId, RetrySafety, RouteTargetId,
    RoutingCredentialId, SettingsConfiguration, UpstreamError, UpstreamErrorClassification,
    UpstreamErrorKind, UpstreamFailureAttribution,
};
use any2api_protocol::api::RequestExecutionProfile;
use any2api_transport::api::{TransportError, TransportErrorStage, TransportFailureScope};
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use tokio::time::Instant;

use super::{
    budget::RetryBudget,
    decision::{RetryDecision, RetryExclusion, exclude_failed_path, retry_decision},
    within_attempt_budget,
};
use crate::{
    credential::{CredentialAuthMaterial, CredentialRuntimeHandle},
    health::EndpointHealthRuntime,
    public_request::upstream::AttemptFailure,
    request_telemetry::AttemptRecorder,
    routing::{CandidateExclusions, RouteCandidate, SchedulerEpoch},
};

#[tokio::test(start_paused = true)]
async fn attempt_result_at_the_deadline_wins_over_the_outer_timeout() {
    let deadline = Instant::now() + Duration::from_millis(25);
    let marker = AttemptRecorder::disabled().timeout_marker();
    let attempt = async {
        tokio::time::sleep_until(deadline).await;
        7
    };

    match within_attempt_budget(deadline, marker, attempt).await {
        Ok(value) => assert_eq!(value, 7),
        Err(_) => panic!("the completed attempt must win at the shared deadline"),
    }
}

#[test]
fn unbound_retry_safe_upstream_failures_choose_their_exact_scope() {
    let candidate = candidate("bad-key");
    let cases = [
        (
            UpstreamFailureAttribution::Unattributed,
            RetryExclusion::ExactCandidate,
        ),
        (
            UpstreamFailureAttribution::Authentication,
            RetryExclusion::Credential,
        ),
        (
            UpstreamFailureAttribution::Credential,
            RetryExclusion::Credential,
        ),
        (
            UpstreamFailureAttribution::CredentialModel,
            RetryExclusion::CredentialModel,
        ),
        (
            UpstreamFailureAttribution::RouteOperation,
            RetryExclusion::RouteOperation,
        ),
        (
            UpstreamFailureAttribution::EgressPath,
            RetryExclusion::EgressPath,
        ),
        (
            UpstreamFailureAttribution::Endpoint,
            RetryExclusion::Endpoint,
        ),
    ];

    for (attribution, expected) in cases {
        let failure = upstream_failure(
            candidate.clone(),
            false,
            UpstreamErrorKind::Authentication,
            RetrySafety::RejectedBeforeExecution,
            attribution,
        );
        let budget = attempted_budget(candidate.credential_id);
        assert_eq!(
            retry_decision(&failure, &budget, candidate.credential_id, false),
            RetryDecision::Reselect(expected),
            "attribution {attribution:?}",
        );
    }
}

#[test]
fn transport_scope_drives_reselection_without_guessing_the_endpoint() {
    let candidate = candidate("transport");
    for (scope, expected) in [
        (TransportFailureScope::Endpoint, RetryExclusion::Endpoint),
        (TransportFailureScope::Proxy, RetryExclusion::Proxy),
        (
            TransportFailureScope::EgressPath,
            RetryExclusion::EgressPath,
        ),
        (
            TransportFailureScope::Unattributed,
            RetryExclusion::ExactCandidate,
        ),
    ] {
        let failure = AttemptFailure::Transport {
            error: Box::new(TransportError::new(
                TransportErrorStage::Tcp,
                scope,
                RetrySafety::DefinitelyNotSent,
                "test failure",
            )),
            candidate: Box::new(candidate.clone()),
            bound: false,
        };
        let budget = attempted_budget(candidate.credential_id);
        assert_eq!(
            retry_decision(&failure, &budget, candidate.credential_id, false),
            RetryDecision::Reselect(expected),
            "scope {scope:?}",
        );
    }
}

#[test]
fn bound_requests_never_switch_credentials_or_targets() {
    let candidate = candidate("bound");
    let deterministic = upstream_failure(
        candidate.clone(),
        true,
        UpstreamErrorKind::Authentication,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::Authentication,
    );
    let budget = attempted_budget(candidate.credential_id);
    assert_eq!(
        retry_decision(&deterministic, &budget, candidate.credential_id, false),
        RetryDecision::Terminal
    );

    let transient = upstream_failure(
        candidate.clone(),
        true,
        UpstreamErrorKind::RateLimited,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::CredentialModel,
    );
    assert!(matches!(
        retry_decision(&transient, &budget, candidate.credential_id, false),
        RetryDecision::RetrySamePath(_)
    ));
}

#[test]
fn ambiguous_server_failure_is_terminal_even_for_an_unbound_request() {
    let candidate = candidate("ambiguous");
    let failure = upstream_failure(
        candidate.clone(),
        false,
        UpstreamErrorKind::Transient,
        RetrySafety::Ambiguous,
        UpstreamFailureAttribution::Unattributed,
    );
    let budget = attempted_budget(candidate.credential_id);

    assert_eq!(
        retry_decision(&failure, &budget, candidate.credential_id, false),
        RetryDecision::Terminal
    );
}

#[test]
fn oauth_authentication_gets_one_refresh_decision_before_reselection() {
    let mut candidate = candidate("oauth");
    let account_id = OAuthAccountId::new();
    candidate.credential_id = RoutingCredentialId::oauth_account(account_id);
    let failure = upstream_failure(
        candidate.clone(),
        false,
        UpstreamErrorKind::Authentication,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::Authentication,
    );
    let budget = attempted_budget(candidate.credential_id);

    assert_eq!(
        retry_decision(&failure, &budget, candidate.credential_id, true),
        RetryDecision::OAuthRefresh {
            account_id,
            token_version: candidate.binding.generation().authentication_version(),
        }
    );
    assert_eq!(
        retry_decision(&failure, &budget, candidate.credential_id, false),
        RetryDecision::Reselect(RetryExclusion::Credential)
    );
}

#[test]
fn credential_model_exclusion_keeps_other_models_and_keys_available() {
    let failed = candidate("failed-model");
    let mut same_credential_other_model = failed.clone();
    same_credential_other_model.upstream_model = "other-model".into();
    same_credential_other_model.target_id = RouteTargetId::new();
    let other_credential = candidate("other-key");
    let failure = upstream_failure(
        failed.clone(),
        false,
        UpstreamErrorKind::ModelUnavailable,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::CredentialModel,
    );
    let mut exclusions = CandidateExclusions::default();

    exclude_failed_path(&mut exclusions, &failure, RetryExclusion::CredentialModel);

    assert!(!exclusions.allows(&failed));
    assert!(exclusions.allows(&same_credential_other_model));
    assert!(exclusions.allows(&other_credential));
}

#[test]
fn bad_key_exclusion_keeps_another_key_on_the_same_endpoint_available() {
    let failed = candidate("bad-key");
    let mut good = candidate("good-key");
    good.endpoint_id = failed.endpoint_id;
    good.endpoint_config_version = failed.endpoint_config_version;
    good.proxy_id = failed.proxy_id;
    good.proxy_config_version = failed.proxy_config_version;
    let failure = upstream_failure(
        failed.clone(),
        false,
        UpstreamErrorKind::Authentication,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::Authentication,
    );
    let budget = attempted_budget(failed.credential_id);
    let decision = retry_decision(&failure, &budget, failed.credential_id, false);
    let RetryDecision::Reselect(scope) = decision else {
        panic!("bad key must trigger safe reselection");
    };
    let mut exclusions = CandidateExclusions::default();

    exclude_failed_path(&mut exclusions, &failure, scope);

    assert!(!exclusions.allows(&failed));
    assert!(exclusions.allows(&good));
}

#[test]
fn egress_exclusion_isolated_by_endpoint_proxy_pair_and_generation() {
    let failed = candidate("bad-egress");
    let mut other_proxy = candidate("other-proxy");
    other_proxy.endpoint_id = failed.endpoint_id;
    other_proxy.endpoint_config_version = failed.endpoint_config_version;
    other_proxy.proxy_id = ProxyProfileId::new();
    let mut other_endpoint = candidate("other-endpoint");
    other_endpoint.proxy_id = failed.proxy_id;
    other_endpoint.proxy_config_version = failed.proxy_config_version;
    let mut new_proxy_generation = failed.clone();
    new_proxy_generation.proxy_config_version += 1;
    let failure = AttemptFailure::Transport {
        error: Box::new(TransportError::new(
            TransportErrorStage::Tls,
            TransportFailureScope::EgressPath,
            RetrySafety::DefinitelyNotSent,
            "egress denied",
        )),
        candidate: Box::new(failed.clone()),
        bound: false,
    };
    let mut exclusions = CandidateExclusions::default();

    exclude_failed_path(&mut exclusions, &failure, RetryExclusion::EgressPath);

    assert!(!exclusions.allows(&failed));
    assert!(exclusions.allows(&other_proxy));
    assert!(exclusions.allows(&other_endpoint));
    assert!(exclusions.allows(&new_proxy_generation));
}

fn upstream_failure(
    candidate: RouteCandidate,
    bound: bool,
    kind: UpstreamErrorKind,
    safety: RetrySafety,
    attribution: UpstreamFailureAttribution,
) -> AttemptFailure {
    AttemptFailure::Upstream {
        status: StatusCode::UNAUTHORIZED,
        headers: Box::new(HeaderMap::new()),
        body: Bytes::new(),
        error: Box::new(UpstreamError::new(
            UpstreamErrorClassification::new(kind, safety, None).with_attribution(attribution),
            None,
        )),
        candidate: Box::new(candidate),
        bound,
    }
}

fn attempted_budget(credential_id: RoutingCredentialId) -> RetryBudget {
    let policy = crate::health::ReliabilityPolicy::from_settings(
        SettingsConfiguration::defaults().reliability(),
    );
    let mut budget = RetryBudget::new(
        policy,
        ProtocolOperation::Responses,
        RequestExecutionProfile::Standard,
    );
    assert_eq!(budget.register_attempt(credential_id), Some(1));
    budget
}

fn candidate(label: &str) -> RouteCandidate {
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
