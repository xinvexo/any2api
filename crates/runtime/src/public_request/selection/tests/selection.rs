use std::{collections::BTreeMap, sync::Arc};

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, ProtocolDialect, ProviderBaseUrl,
    ProviderCredential, ProviderCredentialDraft, ProviderEndpointId, ProviderKind, ProxyProfileId,
    RequestsPerMinute, RetrySafety, RouteTargetId, UpstreamErrorClassification, UpstreamErrorKind,
};

use super::super::{
    GenerationSelection, RouteCandidate, SelectedCandidate, try_select_fixed_candidate_for_test,
    try_select_generation_candidate_for_test,
};
use super::super::{filter_recorder::RequestFilterRecorder, fixed};

use crate::{
    credential::{CredentialAuthMaterial, CredentialRuntimeHandle},
    health::{EndpointHealthRuntime, ReliabilityPolicy},
    routing::SchedulerEpoch,
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
async fn selection_retries_a_raced_half_open_probe_without_consuming_rpm() {
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
    let epoch_before_selection = epoch.current();

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
    assert_eq!(raced.binding.rate_snapshot().requests_in_window(), 0);
    assert!(epoch.current() > epoch_before_selection);
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

#[tokio::test(start_paused = true)]
async fn open_circuit_tier_fails_over_to_the_next_tier() {
    let epoch = SchedulerEpoch::new();
    let policy = default_reliability_policy();
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    open_endpoint(&endpoint, &policy);

    let mut primary = candidate("primary", 1, Arc::clone(&epoch), 0);
    primary.endpoint_health = Some(endpoint);
    let fallback = candidate("fallback", 2, Arc::clone(&epoch), 1);
    let tiers = BTreeMap::from([(0, vec![primary.clone()]), (1, vec![fallback.clone()])]);

    let selected = match try_select_generation_candidate_for_test(false, &tiers, |_| Some(0))
        .expect("generation selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        GenerationSelection::RateLimited(_) => panic!("fallback RPM is available"),
        GenerationSelection::NoCandidates => panic!("fallback candidate exists"),
        GenerationSelection::TemporarilyUnavailable(_) => {
            panic!("the open primary tier must fail over to the healthy tier")
        }
    };
    assert_eq!(selected.candidate.credential_id, fallback.credential_id);
    drop(selected);
}

#[tokio::test(start_paused = true)]
async fn fully_open_tiers_report_temporary_unavailability() {
    let epoch = SchedulerEpoch::new();
    let policy = default_reliability_policy();

    let mut primary = candidate("primary", 1, Arc::clone(&epoch), 0);
    let primary_endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    open_endpoint(&primary_endpoint, &policy);
    primary.endpoint_health = Some(primary_endpoint);
    let mut fallback = candidate("fallback", 2, Arc::clone(&epoch), 1);
    let fallback_endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    open_endpoint(&fallback_endpoint, &policy);
    fallback.endpoint_health = Some(fallback_endpoint);
    let tiers = BTreeMap::from([(0, vec![primary]), (1, vec![fallback])]);

    assert!(matches!(
        try_select_generation_candidate_for_test(false, &tiers, |_| Some(0)),
        Ok(GenerationSelection::TemporarilyUnavailable(_))
    ));
}

#[tokio::test(start_paused = true)]
async fn rate_limit_cooldown_waits_unless_fallback_is_enabled() {
    let epoch = SchedulerEpoch::new();
    let policy = default_reliability_policy();
    let primary = candidate("cooldown", 1, Arc::clone(&epoch), 0);
    primary.binding.generation().health().record(
        &primary.upstream_model,
        UpstreamErrorClassification::new(
            UpstreamErrorKind::RateLimited,
            RetrySafety::RejectedBeforeExecution,
            None,
        ),
        &policy,
    );
    let fallback = candidate("fallback", 2, Arc::clone(&epoch), 1);
    let tiers = BTreeMap::from([(0, vec![primary.clone()]), (1, vec![fallback.clone()])]);

    assert!(matches!(
        try_select_generation_candidate_for_test(false, &tiers, |_| Some(0)),
        Ok(GenerationSelection::TemporarilyUnavailable(_))
    ));
    let selected = match try_select_generation_candidate_for_test(true, &tiers, |_| Some(0))
        .expect("generation selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        GenerationSelection::RateLimited(_) => panic!("fallback RPM is available"),
        GenerationSelection::NoCandidates => panic!("fallback candidate exists"),
        GenerationSelection::TemporarilyUnavailable(_) => {
            panic!("fallback-on-rate-limit must spill the cooldown to the next tier")
        }
    };
    assert_eq!(selected.candidate.credential_id, fallback.credential_id);
    drop(selected);
}

#[tokio::test(start_paused = true)]
async fn quota_exhaustion_cooldown_waits_instead_of_failing_over() {
    let epoch = SchedulerEpoch::new();
    let primary = candidate("quota", 1, Arc::clone(&epoch), 0);
    primary
        .binding
        .generation()
        .health()
        .record_quota_exhaustion(std::time::Duration::from_secs(30), None, None);
    let fallback = candidate("fallback", 2, Arc::clone(&epoch), 1);
    let tiers = BTreeMap::from([(0, vec![primary]), (1, vec![fallback])]);

    assert!(matches!(
        try_select_generation_candidate_for_test(false, &tiers, |_| Some(0)),
        Ok(GenerationSelection::TemporarilyUnavailable(_))
    ));
}

#[test]
fn fixed_selection_records_the_successful_selection() {
    let epoch = SchedulerEpoch::new();
    let candidate = candidate("fixed", 5, Arc::clone(&epoch), 0);
    let selected =
        try_select_fixed_candidate_for_test(default_reliability_policy(), &candidate, || {})
            .expect("fixed selection")
            .expect("fixed RPM reservation");

    assert_eq!(candidate.binding.balancing_counters().selected(), 1);
    assert_eq!(candidate.binding.in_flight(), 1);
    assert_eq!(candidate.binding.rate_snapshot().requests_in_window(), 1);
    drop(selected);
    assert_eq!(candidate.binding.in_flight(), 0);
    assert_eq!(candidate.binding.rate_snapshot().requests_in_window(), 1);
}

#[test]
fn fixed_rechecks_record_one_rate_filter_per_request() {
    let epoch = SchedulerEpoch::new();
    let candidate = candidate("fixed-filter", 7, Arc::clone(&epoch), 0);
    drop(candidate.binding.try_reserve().expect("exhaust RPM"));
    let mut filters = RequestFilterRecorder::default();

    for _ in 0..5 {
        assert!(
            fixed::try_selected_with_recorder_for_test(
                default_reliability_policy(),
                &candidate,
                &mut filters,
            )
            .expect("fixed selection")
            .is_none()
        );
    }

    assert_eq!(
        candidate.binding.balancing_counters().filtered_rate_limit(),
        1
    );
}

#[tokio::test(start_paused = true)]
async fn fixed_selection_rolls_back_rpm_when_the_half_open_probe_is_raced() {
    let epoch = SchedulerEpoch::new();
    let policy = default_reliability_policy();
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    open_endpoint(&endpoint, &policy);
    tokio::time::advance(policy.endpoint_open_duration).await;

    let mut candidate = candidate("fixed-raced", 6, Arc::clone(&epoch), 0);
    candidate.endpoint_health = Some(endpoint);
    let candidate_for_probe = candidate.clone();
    let mut occupied_probe = None;

    let selected = try_select_fixed_candidate_for_test(policy, &candidate, || {
        occupied_probe = Some(
            candidate_for_probe
                .acquire_health(policy)
                .expect("half-open probe"),
        );
    })
    .expect("fixed selection result");

    assert!(selected.is_none());
    assert_eq!(candidate.binding.in_flight(), 0);
    assert_eq!(candidate.binding.rate_snapshot().requests_in_window(), 0);
    assert_eq!(
        candidate
            .binding
            .balancing_counters()
            .filtered_endpoint_health(),
        1
    );
    drop(occupied_probe);
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
    );
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
