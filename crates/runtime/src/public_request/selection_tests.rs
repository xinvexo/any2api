#[path = "selection_tests/queue_tests.rs"]
mod queue_tests;

use std::{collections::BTreeMap, sync::Arc};

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, MaxConcurrency, ProtocolDialect,
    ProviderBaseUrl, ProviderCredential, ProviderCredentialDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId, PublicErrorCode, RouteTargetId,
};

use super::{
    GenerationSelection, RequestPermit, RouteCandidate, SelectedCandidate,
    select_auxiliary_candidate_for_test, try_select_fixed_candidate_for_test,
    try_select_generation_candidate_for_test,
};

use crate::{
    auxiliary_scheduler::{
        AuxiliaryConcurrencyLimits, AuxiliaryScheduler, AuxiliarySelectAndAcquireResult,
    },
    credential_auth::CredentialAuthMaterial,
    credential_runtime::CredentialRuntimeHandle,
    health::{EndpointHealthRuntime, ReliabilityPolicy},
    scheduler_epoch::SchedulerEpoch,
};

#[test]
fn auxiliary_saturation_does_not_fall_through_to_a_later_tier() {
    let epoch = SchedulerEpoch::new();
    let scheduler = AuxiliaryScheduler::new(
        AuxiliaryConcurrencyLimits::new(1, 1).expect("limits"),
        Arc::clone(&epoch),
    );
    let primary = candidate("primary", 1, Arc::clone(&epoch), 0);
    let fallback = candidate("fallback", 2, Arc::clone(&epoch), 1);
    let primary_slot =
        match scheduler.select_index_and_try_acquire(std::slice::from_ref(&primary.binding), 0) {
            AuxiliarySelectAndAcquireResult::Acquired { permit, .. } => permit,
            AuxiliarySelectAndAcquireResult::AtCapacity => panic!("primary slot available"),
            AuxiliarySelectAndAcquireResult::NoCandidates => panic!("primary candidate exists"),
        };
    let tiers = BTreeMap::from([(0, vec![primary]), (1, vec![fallback.clone()])]);

    let error = match select_auxiliary_candidate_for_test(&scheduler, &tiers, |_| Some(0)) {
        Ok(_) => panic!("primary saturation must fail immediately"),
        Err(error) => error,
    };

    assert_eq!(error.code, PublicErrorCode::LocalConcurrencyLimit);
    assert_eq!(fallback.binding.auxiliary_in_flight(), 0);
    drop(primary_slot);
}

#[test]
fn generation_fallback_only_skips_a_saturated_tier_when_enabled() {
    let epoch = SchedulerEpoch::new();
    let primary = candidate("primary", 1, Arc::clone(&epoch), 0);
    let fallback = candidate("fallback", 2, Arc::clone(&epoch), 1);
    let blocker = primary.binding.try_acquire().expect("primary blocker");
    let tiers = BTreeMap::from([(0, vec![primary.clone()]), (1, vec![fallback.clone()])]);

    assert!(matches!(
        try_select_generation_candidate_for_test(false, &tiers, |_| Some(0)),
        Ok(GenerationSelection::AtCapacity)
    ));
    let selected = match try_select_generation_candidate_for_test(true, &tiers, |_| Some(0))
        .expect("generation selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        GenerationSelection::AtCapacity => panic!("fallback capacity is available"),
        GenerationSelection::NoCandidates => panic!("fallback candidate exists"),
        GenerationSelection::TemporarilyUnavailable(_) => {
            panic!("fallback candidate is healthy")
        }
    };
    assert_eq!(selected.candidate.credential_id, fallback.credential_id);
    assert_eq!(primary.binding.balancing_counters().filtered_capacity(), 2);
    assert_eq!(
        fallback.binding.balancing_counters().selected_generation(),
        1
    );
    drop(selected);
    drop(blocker);
}

#[tokio::test(start_paused = true)]
async fn generation_selection_retries_the_tier_when_a_half_open_probe_is_raced() {
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
        GenerationSelection::AtCapacity => panic!("healthy candidate has capacity"),
        GenerationSelection::TemporarilyUnavailable(_) => {
            panic!("healthy candidate must be retried in the same tier")
        }
        GenerationSelection::NoCandidates => panic!("healthy candidate exists"),
    };

    assert_eq!(selected.candidate.credential_id, healthy.credential_id);
    assert_eq!(raced.binding.capacity().in_flight(), 0);
    assert_eq!(
        raced
            .binding
            .balancing_counters()
            .filtered_endpoint_health(),
        1
    );
    assert_eq!(
        healthy.binding.balancing_counters().selected_generation(),
        1
    );
    drop(selected);
    drop(occupied_probe);
}

#[tokio::test(start_paused = true)]
async fn auxiliary_selection_retries_the_tier_when_a_half_open_probe_is_raced() {
    let epoch = SchedulerEpoch::new();
    let policy = default_reliability_policy();
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    open_endpoint(&endpoint, &policy);
    tokio::time::advance(policy.endpoint_open_duration).await;
    let scheduler = AuxiliaryScheduler::new(
        AuxiliaryConcurrencyLimits::new(2, 1).expect("limits"),
        Arc::clone(&epoch),
    );

    let mut raced = candidate("aux-raced", 3, Arc::clone(&epoch), 0);
    raced.endpoint_health = Some(endpoint);
    let healthy = candidate("aux-healthy", 4, Arc::clone(&epoch), 0);
    let raced_for_probe = raced.clone();
    let tiers = BTreeMap::from([(0, vec![raced.clone(), healthy.clone()])]);
    let mut occupied_probe = None;

    let selected = select_auxiliary_candidate_for_test(&scheduler, &tiers, |_| {
        if occupied_probe.is_none() {
            occupied_probe = Some(
                raced_for_probe
                    .acquire_health(policy)
                    .expect("half-open probe"),
            );
        }
        Some(0)
    })
    .expect("auxiliary selection");

    assert_eq!(selected.candidate.credential_id, healthy.credential_id);
    assert_eq!(raced.binding.auxiliary_in_flight(), 0);
    assert_eq!(
        raced
            .binding
            .balancing_counters()
            .filtered_endpoint_health(),
        1
    );
    assert_eq!(healthy.binding.balancing_counters().selected_auxiliary(), 1);
    drop(selected);
    drop(occupied_probe);
}

#[test]
fn generation_selection_reports_no_candidates_for_empty_tiers() {
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
        .expect("fixed capacity");

    assert_eq!(
        candidate.binding.balancing_counters().selected_generation(),
        1
    );
    drop(selected);
}

pub(super) fn try_acquire_candidate(
    candidate: &RouteCandidate,
) -> Result<GenerationSelection, any2api_domain::PublicError> {
    let Some(permit) = candidate.binding.try_acquire() else {
        return Ok(GenerationSelection::AtCapacity);
    };
    Ok(GenerationSelection::Acquired(Box::new(SelectedCandidate {
        candidate: candidate.clone(),
        permit: RequestPermit::Generation(permit),
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
            MaxConcurrency::new(1).expect("max concurrency"),
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
