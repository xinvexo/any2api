use std::{collections::BTreeMap, sync::Arc};

use any2api_domain::{
    ModelRouteId, ProtocolDialect, ProtocolOperation, RetrySafety, UpstreamError,
    UpstreamErrorClassification, UpstreamErrorKind, UpstreamFailureAttribution,
};

use super::super::{
    GenerationSelection, tier::try_preferred, try_select_generation_candidate_for_test,
};
use super::selection::{candidate, default_reliability_policy, unlimited_candidate};
use crate::{
    public_request::upstream::AttemptFailure,
    routing::{CacheLocalityRegistry, CacheLocalityTarget, CandidateExclusions, SchedulerEpoch},
};

#[test]
fn cache_locality_prefers_the_previous_success_across_tiers() {
    let epoch = SchedulerEpoch::new();
    let primary = unlimited_candidate("primary", 1, Arc::clone(&epoch), 0);
    let cached = unlimited_candidate("cached", 2, Arc::clone(&epoch), 1);
    let tiers = BTreeMap::from([(0, vec![primary]), (1, vec![cached.clone()])]);

    let selected = try_preferred(
        default_reliability_policy(),
        &tiers,
        &CandidateExclusions::default(),
        &CacheLocalityTarget::from_candidate(&cached),
    )
    .expect("cached path is immediately available");

    assert_eq!(selected.candidate.credential_id, cached.credential_id);
}

#[test]
fn unavailable_cache_locality_falls_back_to_normal_selection() {
    let epoch = SchedulerEpoch::new();
    let cached = candidate("cached", 1, Arc::clone(&epoch), 1);
    drop(cached.binding.try_reserve().expect("exhaust cached RPM"));
    let primary = unlimited_candidate("primary", 2, Arc::clone(&epoch), 0);
    let tiers = BTreeMap::from([(0, vec![primary.clone()]), (1, vec![cached.clone()])]);

    assert!(
        try_preferred(
            default_reliability_policy(),
            &tiers,
            &CandidateExclusions::default(),
            &CacheLocalityTarget::from_candidate(&cached),
        )
        .is_none()
    );
    let selected = match try_select_generation_candidate_for_test(false, &tiers, |_| Some(0))
        .expect("normal selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        _ => panic!("normal pool has an available primary candidate"),
    };
    assert_eq!(selected.candidate.credential_id, primary.credential_id);
}

#[test]
fn retry_exclusions_block_a_cached_candidate() {
    let epoch = SchedulerEpoch::new();
    let cached = unlimited_candidate("cached", 1, Arc::clone(&epoch), 0);
    let tiers = BTreeMap::from([(0, vec![cached.clone()])]);
    let mut exclusions = CandidateExclusions::default();
    exclusions.exclude_candidate(&cached);

    assert!(
        try_preferred(
            default_reliability_policy(),
            &tiers,
            &exclusions,
            &CacheLocalityTarget::from_candidate(&cached),
        )
        .is_none()
    );
}

#[tokio::test(start_paused = true)]
async fn permission_denied_path_does_not_return_after_cooldown_for_the_same_cache_key() {
    let epoch = SchedulerEpoch::new();
    let policy = default_reliability_policy();
    let custom = unlimited_candidate("custom", 1, Arc::clone(&epoch), 0);
    let oauth = unlimited_candidate("oauth", 2, Arc::clone(&epoch), 1);
    let tiers = BTreeMap::from([(0, vec![custom.clone()]), (1, vec![oauth.clone()])]);
    let registry = CacheLocalityRegistry::new();
    let key = registry.key(
        ProtocolDialect::OpenAiResponses,
        ProtocolOperation::Responses,
        ModelRouteId::new(),
        "conversation-cache-key",
    );
    registry.remember_candidate(key, &custom);

    let classification = UpstreamErrorClassification::new(
        UpstreamErrorKind::PermissionDenied,
        RetrySafety::RejectedBeforeExecution,
        None,
    )
    .with_attribution(UpstreamFailureAttribution::Credential);
    let failure = AttemptFailure::Upstream {
        status: http::StatusCode::FORBIDDEN,
        headers: Box::new(http::HeaderMap::new()),
        body: bytes::Bytes::new(),
        error: Box::new(UpstreamError::new(classification, None)),
        candidate: Box::new(custom.clone()),
        bound: false,
    };
    registry.forget_candidate(
        key,
        failure
            .cache_locality_failure_candidate()
            .expect("403 implicates the selected path"),
    );
    assert!(registry.lookup(key).is_none());

    custom
        .binding
        .generation()
        .health()
        .record(&custom.upstream_model, classification, &policy);
    let selected = match try_select_generation_candidate_for_test(false, &tiers, |_| Some(0))
        .expect("normal fallback selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        _ => panic!("OAuth fallback is available while the custom path cools down"),
    };
    assert_eq!(selected.candidate.credential_id, oauth.credential_id);
    registry.remember_candidate(key, &selected.candidate);
    drop(selected);

    tokio::time::advance(policy.permission_denied + std::time::Duration::from_secs(1)).await;
    let ordinary = match try_select_generation_candidate_for_test(false, &tiers, |_| Some(0))
        .expect("ordinary selection after cooldown")
    {
        GenerationSelection::Acquired(selected) => selected,
        _ => panic!("custom path is healthy again after cooldown"),
    };
    assert_eq!(ordinary.candidate.credential_id, custom.credential_id);
    drop(ordinary);

    let remembered = registry.lookup(key).expect("OAuth success was remembered");
    let selected = try_preferred(policy, &tiers, &CandidateExclusions::default(), &remembered)
        .expect("remembered OAuth path remains available");
    assert_eq!(selected.candidate.credential_id, oauth.credential_id);
}
