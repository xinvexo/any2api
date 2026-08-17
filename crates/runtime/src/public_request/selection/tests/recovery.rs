use std::{collections::BTreeMap, sync::Arc, time::Duration};

use any2api_domain::{ProviderEndpointId, RouteTargetId};

use super::selection::{candidate, default_reliability_policy, open_endpoint};
use crate::{
    health::EndpointHealthRuntime,
    public_request::selection::{
        GenerationSelection, try_select_generation_candidate_with_state_for_test,
    },
    routing::{CandidateFailureScope, CandidateSelectionState, SchedulerEpoch},
};

#[test]
fn retry_preference_orders_all_unattempted_combinations_before_the_failed_exact_path() {
    let epoch = SchedulerEpoch::new();
    let failed = candidate("failed", 1, Arc::clone(&epoch), 0);
    let new_credential_new_egress = candidate("new-new", 2, Arc::clone(&epoch), 0);
    let mut new_credential_old_egress = candidate("new-old", 3, Arc::clone(&epoch), 0);
    new_credential_old_egress.endpoint_id = failed.endpoint_id;
    new_credential_old_egress.endpoint_config_version = failed.endpoint_config_version;
    new_credential_old_egress.proxy_id = failed.proxy_id;
    new_credential_old_egress.proxy_config_version = failed.proxy_config_version;
    let mut old_credential_new_egress = failed.clone();
    old_credential_new_egress.target_id = RouteTargetId::new();
    old_credential_new_egress.endpoint_id = ProviderEndpointId::new();
    let mut old_credential_old_egress = failed.clone();
    old_credential_old_egress.target_id = RouteTargetId::new();
    let mut state = CandidateSelectionState::default();
    state.note_failed(&failed);

    assert_eq!(state.retry_preference(&new_credential_new_egress), 0);
    assert_eq!(state.retry_preference(&new_credential_old_egress), 1);
    assert_eq!(state.retry_preference(&old_credential_new_egress), 2);
    assert_eq!(state.retry_preference(&old_credential_old_egress), 3);
    assert_eq!(state.retry_preference(&failed), 4);
}

#[test]
fn unattempted_exact_candidate_precedes_the_failed_exact_path() {
    let epoch = SchedulerEpoch::new();
    let failed = candidate("failed-exact", 1, Arc::clone(&epoch), 0);
    let mut alternate = failed.clone();
    alternate.target_id = RouteTargetId::new();
    alternate.candidate_path_health = EndpointHealthRuntime::new(Arc::clone(&epoch));
    let tiers = BTreeMap::from([(0, vec![failed.clone(), alternate.clone()])]);
    let mut state = CandidateSelectionState::default();
    state.note_failed(&failed);

    let selected = match try_select_generation_candidate_with_state_for_test(
        false,
        &tiers,
        &state,
        &|_| true,
        |_| Some(0),
    )
    .expect("generation selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        _ => panic!("the unattempted exact candidate must be selected"),
    };

    assert_eq!(selected.candidate.target_id, alternate.target_id);
    drop(selected);
}

#[tokio::test(start_paused = true)]
async fn deferred_candidate_does_not_reserve_rpm_or_acquire_a_guard() {
    let epoch = SchedulerEpoch::new();
    let candidate = candidate("deferred", 1, epoch, 0);
    let tiers = BTreeMap::from([(0, vec![candidate.clone()])]);
    let retry_at = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut state = CandidateSelectionState::default();
    state.note_failed(&candidate);
    state.defer(&candidate, CandidateFailureScope::ExactCandidate, retry_at);

    assert!(matches!(
        try_select_generation_candidate_with_state_for_test(
            false,
            &tiers,
            &state,
            &|_| true,
            |_| Some(0),
        ),
        Ok(GenerationSelection::RetryDeferred(deadline)) if deadline == retry_at
    ));
    assert_eq!(candidate.binding.rate_snapshot().requests_in_window(), 0);
    assert_eq!(candidate.binding.in_flight(), 0);
    assert_eq!(candidate.binding.balancing_counters().selected(), 0);
}

#[tokio::test(start_paused = true)]
async fn retry_deferred_wakes_at_an_earlier_rpm_deadline() {
    let epoch = SchedulerEpoch::new();
    let deferred = candidate("deferred-later", 1, Arc::clone(&epoch), 0);
    let rate_limited = candidate("rpm-earlier", 2, epoch, 0);
    drop(rate_limited.binding.try_reserve().expect("exhaust RPM"));
    tokio::time::advance(Duration::from_secs(55)).await;
    let rpm_retry_at = rate_limited
        .binding
        .rate_snapshot()
        .retry_at()
        .expect("finite RPM window has a retry deadline");
    let mut state = CandidateSelectionState::default();
    state.note_failed(&deferred);
    state.defer(
        &deferred,
        CandidateFailureScope::ExactCandidate,
        tokio::time::Instant::now() + Duration::from_secs(30),
    );
    let tiers = BTreeMap::from([(0, vec![deferred, rate_limited])]);

    assert!(matches!(
        try_select_generation_candidate_with_state_for_test(
            false,
            &tiers,
            &state,
            &|_| true,
            |_| Some(0),
        ),
        Ok(GenerationSelection::RetryDeferred(deadline)) if deadline == rpm_retry_at
    ));
}

#[tokio::test(start_paused = true)]
async fn same_tier_rate_limit_preserves_an_earlier_health_deadline() {
    let epoch = SchedulerEpoch::new();
    let rate_limited = candidate("rpm-later", 1, Arc::clone(&epoch), 0);
    drop(rate_limited.binding.try_reserve().expect("exhaust RPM"));
    let policy = default_reliability_policy();
    assert!(policy.endpoint_open_duration < Duration::from_secs(60));
    let endpoint_health = EndpointHealthRuntime::new(Arc::clone(&epoch));
    open_endpoint(&endpoint_health, &policy);
    let health_retry_at = tokio::time::Instant::now() + policy.endpoint_open_duration;
    let mut unhealthy = candidate("health-earlier", 2, epoch, 0);
    unhealthy.endpoint_health = Some(endpoint_health);
    let tiers = BTreeMap::from([(0, vec![rate_limited, unhealthy])]);

    assert!(matches!(
        try_select_generation_candidate_with_state_for_test(
            false,
            &tiers,
            &CandidateSelectionState::default(),
            &|_| true,
            |_| Some(0),
        ),
        Ok(GenerationSelection::RateLimited(Some(deadline))) if deadline == health_retry_at
    ));
}

#[tokio::test(start_paused = true)]
async fn single_deferred_candidate_reenters_selection_at_its_latest_scope_deadline() {
    let epoch = SchedulerEpoch::new();
    let candidate = candidate("retry-at", 1, epoch, 0);
    let tiers = BTreeMap::from([(0, vec![candidate.clone()])]);
    let now = tokio::time::Instant::now();
    let retry_at = now + Duration::from_secs(5);
    let mut state = CandidateSelectionState::default();
    state.note_failed(&candidate);
    state.defer(
        &candidate,
        CandidateFailureScope::ExactCandidate,
        now + Duration::from_secs(2),
    );
    state.defer(&candidate, CandidateFailureScope::Endpoint, retry_at);
    state.defer(
        &candidate,
        CandidateFailureScope::Endpoint,
        now + Duration::from_secs(3),
    );

    tokio::time::advance(Duration::from_secs(2)).await;
    assert!(matches!(
        try_select_generation_candidate_with_state_for_test(
            false,
            &tiers,
            &state,
            &|_| true,
            |_| Some(0),
        ),
        Ok(GenerationSelection::RetryDeferred(deadline)) if deadline == retry_at
    ));
    tokio::time::advance(Duration::from_secs(3)).await;
    let selected = match try_select_generation_candidate_with_state_for_test(
        false,
        &tiers,
        &state,
        &|_| true,
        |_| Some(0),
    )
    .expect("generation selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        _ => panic!("the candidate must be eligible at not-before"),
    };
    drop(selected);
}

#[tokio::test(start_paused = true)]
async fn mixed_primary_blockers_do_not_delay_an_immediate_fallback() {
    let epoch = SchedulerEpoch::new();
    let deferred = candidate("deferred-primary", 1, Arc::clone(&epoch), 0);
    let rate_limited = candidate("rpm-primary", 2, Arc::clone(&epoch), 0);
    drop(rate_limited.binding.try_reserve().expect("exhaust RPM"));
    let mut unhealthy = candidate("unhealthy-primary", 3, Arc::clone(&epoch), 0);
    let endpoint_health = EndpointHealthRuntime::new(Arc::clone(&epoch));
    open_endpoint(&endpoint_health, &default_reliability_policy());
    unhealthy.endpoint_health = Some(endpoint_health);
    let fallback = candidate("fallback", 4, Arc::clone(&epoch), 1);
    let tiers = BTreeMap::from([
        (0, vec![deferred.clone(), rate_limited, unhealthy]),
        (1, vec![fallback.clone()]),
    ]);
    let mut state = CandidateSelectionState::default();
    state.note_failed(&deferred);
    state.defer(
        &deferred,
        CandidateFailureScope::ExactCandidate,
        tokio::time::Instant::now() + Duration::from_secs(30),
    );

    let selected = match try_select_generation_candidate_with_state_for_test(
        false,
        &tiers,
        &state,
        &|_| true,
        |_| Some(0),
    )
    .expect("generation selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        _ => panic!("healthy fallback must not inherit request-local deferral"),
    };
    assert_eq!(selected.candidate.credential_id, fallback.credential_id);
    assert_eq!(deferred.binding.rate_snapshot().requests_in_window(), 0);
    drop(selected);
}

#[test]
fn credential_eligibility_is_checked_before_rpm_reservation() {
    let epoch = SchedulerEpoch::new();
    let ineligible = candidate("budget-ineligible", 1, Arc::clone(&epoch), 0);
    let eligible = candidate("budget-eligible", 2, epoch, 0);
    let tiers = BTreeMap::from([(0, vec![ineligible.clone(), eligible.clone()])]);

    let selected = match try_select_generation_candidate_with_state_for_test(
        false,
        &tiers,
        &CandidateSelectionState::default(),
        &|credential_id| credential_id == eligible.credential_id,
        |_| Some(0),
    )
    .expect("generation selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        _ => panic!("eligible credential must be selected"),
    };
    assert_eq!(selected.candidate.credential_id, eligible.credential_id);
    assert_eq!(ineligible.binding.rate_snapshot().requests_in_window(), 0);
    assert_eq!(ineligible.binding.in_flight(), 0);
    drop(selected);
}
