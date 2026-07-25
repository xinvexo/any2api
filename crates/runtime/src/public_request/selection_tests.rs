#[path = "selection_tests/queue_tests.rs"]
mod queue_tests;

use std::{collections::BTreeMap, sync::Arc};

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, ProtocolDialect, ProviderBaseUrl,
    ProviderCredential, ProviderCredentialDraft, ProviderEndpointId, ProviderKind, ProxyProfileId,
    RequestsPerMinute, RouteTargetId,
};

use super::{
    GenerationSelection, RouteCandidate, SelectedCandidate, try_select_fixed_candidate_for_test,
    try_select_generation_candidate_for_test,
};

use crate::{
    credential_auth::CredentialAuthMaterial,
    credential_runtime::CredentialRuntimeHandle,
    health::{EndpointHealthRuntime, ReliabilityPolicy},
    scheduler_epoch::SchedulerEpoch,
};

#[test]
fn fallback_only_skips_a_rate_limited_tier_when_enabled() {
    let epoch = SchedulerEpoch::new();
    let primary = candidate("primary", 1, Arc::clone(&epoch), 0);
    let fallback = candidate("fallback", 2, Arc::clone(&epoch), 1);
    drop(primary.binding.try_reserve().expect("exhaust primary RPM"));
    let tiers = BTreeMap::from([(0, vec![primary.clone()]), (1, vec![fallback.clone()])]);

    assert!(matches!(
        try_select_generation_candidate_for_test(false, &tiers, |_| Some(0)),
        Ok(GenerationSelection::RateLimited(Some(_)))
    ));
    let selected = match try_select_generation_candidate_for_test(true, &tiers, |_| Some(0))
        .expect("generation selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        GenerationSelection::RateLimited(_) => panic!("fallback RPM is available"),
        GenerationSelection::NoCandidates => panic!("fallback candidate exists"),
        GenerationSelection::TemporarilyUnavailable(_) => {
            panic!("fallback candidate is healthy")
        }
    };
    assert_eq!(selected.candidate.credential_id, fallback.credential_id);
    assert_eq!(
        primary.binding.balancing_counters().filtered_rate_limit(),
        2
    );
    assert_eq!(fallback.binding.balancing_counters().selected(), 1);
    drop(selected);
}

#[tokio::test(start_paused = true)]
async fn selection_retries_the_tier_when_a_half_open_probe_is_raced() {
    let epoch = SchedulerEpoch::new();
    let policy = default_reliability_policy();
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    open_endpoint(&endpoint, &policy);
    tokio::time::advance(policy.endpoint_open_duration).await;

    let mut raced = candidate("raced", 1, Arc::clone(&epoch), 0);
    raced.endpoint_health = Some(endpoint);
    let healthy = candidate("healthy", 2, Arc::clone(&epoch), 0);
    let raced_for_probe = raced.clone();
    let tiers = BTreeMap::from([(0, vec![raced.clone(), healthy.clone()])]);
    let mut occupied_probe = None;

    let selected = match try_select_generation_candidate_for_test(false, &tiers, |_| {
        if occupied_probe.is_none() {
            occupied_probe = Some(
                raced_for_probe
                    .acquire_health(policy)
                    .expect("half-open probe"),
            );
        }
        Some(0)
    })
    .expect("generation selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        GenerationSelection::RateLimited(_) => panic!("healthy candidate has RPM"),
        GenerationSelection::TemporarilyUnavailable(_) => {
            panic!("healthy candidate must be retried in the same tier")
        }
        GenerationSelection::NoCandidates => panic!("healthy candidate exists"),
    };

    assert_eq!(selected.candidate.credential_id, healthy.credential_id);
    assert_eq!(raced.binding.in_flight(), 0);
    assert_eq!(raced.binding.rate_snapshot().requests_in_window(), 1);
    assert_eq!(
        raced
            .binding
            .balancing_counters()
            .filtered_endpoint_health(),
        1
    );
    assert_eq!(healthy.binding.balancing_counters().selected(), 1);
    drop(selected);
    drop(occupied_probe);
}

#[test]
fn selection_reports_no_candidates_for_empty_tiers() {
    let tiers = BTreeMap::new();

    assert!(matches!(
        try_select_generation_candidate_for_test(false, &tiers, |_| Some(0)),
        Ok(GenerationSelection::NoCandidates)
    ));
}

#[test]
fn fixed_selection_records_the_successful_selection() {
    let epoch = SchedulerEpoch::new();
    let candidate = candidate("fixed", 5, Arc::clone(&epoch), 0);
    let selected = try_select_fixed_candidate_for_test(default_reliability_policy(), &candidate)
        .expect("fixed selection")
        .expect("fixed RPM reservation");

    assert_eq!(candidate.binding.balancing_counters().selected(), 1);
    drop(selected);
}

pub(super) fn try_reserve_candidate(
    candidate: &RouteCandidate,
) -> Result<GenerationSelection, any2api_domain::PublicError> {
    let permit = match candidate.binding.try_reserve() {
        Ok(permit) => permit,
        Err(rate_limited) => {
            return Ok(GenerationSelection::RateLimited(rate_limited.retry_at));
        }
    };
    Ok(GenerationSelection::Acquired(Box::new(SelectedCandidate {
        candidate: candidate.clone(),
        permit,
        health: candidate
            .acquire_health(crate::health::ReliabilityPolicy::from_settings(
                any2api_domain::SettingsConfiguration::defaults().reliability(),
            ))
            .map_err(|_| crate::public_request::response::internal_error())?,
    })))
}

pub(super) fn default_reliability_policy() -> ReliabilityPolicy {
    ReliabilityPolicy::from_settings(
        any2api_domain::SettingsConfiguration::defaults().reliability(),
    )
}

fn open_endpoint(endpoint: &Arc<EndpointHealthRuntime>, policy: &ReliabilityPolicy) {
    let permits = (0..policy.endpoint_failure_threshold)
        .map(|_| endpoint.try_acquire(policy).expect("closed endpoint"))
        .collect::<Vec<_>>();
    for permit in permits {
        permit.failure(policy);
    }
}

pub(super) fn candidate(
    label: &str,
    fingerprint_byte: u8,
    scheduler_epoch: Arc<SchedulerEpoch>,
    tier: u16,
) -> RouteCandidate {
    let credential = ProviderCredential::create(
        CredentialId::new(),
        ProviderEndpointId::new(),
        ProviderCredentialDraft::new(
            label,
            CredentialKind::ApiKey,
            ProxyProfileId::DIRECT,
            Some(RequestsPerMinute::new(1).expect("valid RPM")),
            true,
        )
        .expect("credential draft"),
        CredentialSecretFingerprint::new([fingerprint_byte; 32], None).expect("fingerprint"),
    );
    let binding = CredentialRuntimeHandle::new_for_provider_test(
        &credential,
        CredentialAuthMaterial::for_test(&credential, format!("sk-{label}-test")),
        scheduler_epoch,
    )
    .current_binding();
    RouteCandidate {
        target_id: RouteTargetId::new(),
        endpoint_id: credential.provider_endpoint_id(),
        credential_id: credential.id().into(),
        provider_kind: ProviderKind::Codex,
        base_url: ProviderBaseUrl::parse("https://api.example.com").expect("base URL"),
        upstream_model: format!("upstream-{tier}"),
        upstream_protocol_dialect: ProtocolDialect::OpenAiResponses,
        proxy_id: ProxyProfileId::DIRECT,
        endpoint_health: None,
        proxy_health: None,
        binding,
    }
}
