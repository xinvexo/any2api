use std::{collections::BTreeMap, sync::Arc};

use any2api_domain::{FallbackTier, ModelRouteId, PublicError, RoutingCredentialId};
use tokio::time::{Instant, sleep_until, timeout_at};

use super::super::SelectedCandidate;
use super::{
    GenerationSelection, SelectionWaitState,
    filter_recorder::RequestFilterRecorder,
    no_available_credentials, rate_limit_error, rate_limited, temporarily_unavailable,
    tier::{self, TierScan},
};
use crate::{
    configuration::PublishedSnapshot,
    health::ReliabilityPolicy,
    routing::{
        CandidateSelectionState, QueueCoordinator, QueuePolicy, RateLimitAction, RouteCandidate,
    },
};

pub(super) fn try_select(
    input: GenerationSelectionInput<'_>,
) -> Result<GenerationSelection, PublicError> {
    let GenerationSelectionInput {
        policy_snapshot,
        routing_snapshot,
        route_id,
        fallback_on_rate_limit,
        tiers,
        selection_state,
        credential_eligible,
        filters,
    } = input;
    try_select_with(
        policy_snapshot.reliability_policy(),
        fallback_on_rate_limit,
        SelectionPool {
            tiers,
            selection_state,
            credential_eligible,
        },
        filters,
        |tier| {
            routing_snapshot
                .route_tier_cursor(route_id, FallbackTier::new(tier))
                .map(|cursor| cursor.reserve())
        },
        |tier, skipped| {
            routing_snapshot
                .route_tier_cursor(route_id, FallbackTier::new(tier))
                .is_some_and(|cursor| {
                    cursor.advance_by(skipped);
                    true
                })
        },
    )
}

pub(super) struct GenerationSelectionInput<'a> {
    pub(super) policy_snapshot: &'a PublishedSnapshot,
    pub(super) routing_snapshot: &'a PublishedSnapshot,
    pub(super) route_id: ModelRouteId,
    pub(super) fallback_on_rate_limit: bool,
    pub(super) tiers: &'a BTreeMap<u16, Vec<RouteCandidate>>,
    pub(super) selection_state: &'a CandidateSelectionState,
    pub(super) credential_eligible: &'a (dyn Fn(RoutingCredentialId) -> bool + Sync),
    pub(super) filters: &'a mut RequestFilterRecorder,
}

struct SelectionPool<'a> {
    tiers: &'a BTreeMap<u16, Vec<RouteCandidate>>,
    selection_state: &'a CandidateSelectionState,
    credential_eligible: &'a (dyn Fn(RoutingCredentialId) -> bool + Sync),
}

fn try_select_with(
    policy: ReliabilityPolicy,
    fallback_on_rate_limit: bool,
    pool: SelectionPool<'_>,
    filters: &mut RequestFilterRecorder,
    mut tie_breaker: impl FnMut(u16) -> Option<u64>,
    mut advance_cursor: impl FnMut(u16, u64) -> bool,
) -> Result<GenerationSelection, PublicError> {
    let mut saw_rate_limit = false;
    let mut rate_retry_at = None;
    let mut skipped_retry_at = None;
    let mut deferred_retry_at = None;
    let now = Instant::now();
    let has_active_deferral = pool.tiers.values().flatten().any(|candidate| {
        candidate.admission_active()
            && pool.selection_state.allows(candidate)
            && (pool.credential_eligible)(candidate.credential_id)
            && pool
                .selection_state
                .deferred_until(candidate)
                .is_some_and(|not_before| not_before > now)
    });
    for (tier, candidates) in pool.tiers {
        let tie_breaker =
            tie_breaker(*tier).ok_or_else(crate::public_request::response::internal_error)?;
        match tier::scan(
            policy,
            candidates,
            pool.selection_state,
            pool.credential_eligible,
            filters,
            tie_breaker,
        ) {
            TierScan::Acquired { selected, skipped } => {
                if !advance_cursor(*tier, skipped) {
                    return Err(crate::public_request::response::internal_error());
                }
                return Ok(GenerationSelection::Acquired(selected));
            }
            TierScan::RateLimited {
                retry_at,
                outage_retry_at,
                cooldown_retry_at,
                deferred_retry_at: tier_deferred_retry_at,
            } => {
                saw_rate_limit = true;
                if let Some(retry_at) = retry_at {
                    rate_retry_at = earliest(rate_retry_at, retry_at);
                }
                if let Some(retry_at) = tier_deferred_retry_at {
                    deferred_retry_at = earliest(deferred_retry_at, retry_at);
                }
                for retry_at in [outage_retry_at, cooldown_retry_at].into_iter().flatten() {
                    skipped_retry_at = earliest(skipped_retry_at, retry_at);
                }
                if !fallback_on_rate_limit && !has_active_deferral {
                    return Ok(GenerationSelection::RateLimited(earliest_optional(
                        rate_retry_at,
                        skipped_retry_at,
                    )));
                }
            }
            TierScan::Exhausted {
                outage_retry_at,
                cooldown_retry_at,
                deferred_retry_at: tier_deferred_retry_at,
            } => {
                if let Some(retry_at) = tier_deferred_retry_at {
                    deferred_retry_at = earliest(deferred_retry_at, retry_at);
                }
                // The whole tier is temporarily blocked. Upstream rate-limit
                // and quota cooldowns keep wait-in-place semantics unless the
                // route explicitly spills them to a lower tier.
                if let Some(retry_at) = cooldown_retry_at
                    && !fallback_on_rate_limit
                    && !has_active_deferral
                {
                    let retry_at = outage_retry_at.map_or(retry_at, |outage| outage.min(retry_at));
                    return Ok(GenerationSelection::TemporarilyUnavailable(retry_at));
                }
                for retry_at in [outage_retry_at, cooldown_retry_at].into_iter().flatten() {
                    skipped_retry_at = earliest(skipped_retry_at, retry_at);
                }
            }
        }
    }
    let earliest_wake_at = [deferred_retry_at, rate_retry_at, skipped_retry_at]
        .into_iter()
        .flatten()
        .min();
    Ok(if deferred_retry_at.is_some() {
        GenerationSelection::RetryDeferred(
            earliest_wake_at.expect("an active deferral always has a known deadline"),
        )
    } else if saw_rate_limit {
        GenerationSelection::RateLimited(earliest_wake_at)
    } else if let Some(retry_at) = skipped_retry_at {
        GenerationSelection::TemporarilyUnavailable(retry_at)
    } else {
        GenerationSelection::NoCandidates
    })
}

pub(super) async fn select_with_queue(
    coordinator: &Arc<QueueCoordinator>,
    policy: QueuePolicy,
    wait_state: &SelectionWaitState,
    mut try_select: impl FnMut() -> Result<GenerationSelection, PublicError>,
) -> Result<SelectedCandidate, PublicError> {
    match try_select()? {
        GenerationSelection::Acquired(selected) => Ok(*selected),
        GenerationSelection::NoCandidates => Err(no_available_credentials()),
        GenerationSelection::TemporarilyUnavailable(retry_at)
            if policy.on_rate_limited() == RateLimitAction::Reject =>
        {
            Err(temporarily_unavailable(retry_at))
        }
        GenerationSelection::RateLimited(retry_at)
            if policy.on_rate_limited() == RateLimitAction::Reject =>
        {
            Err(rate_limited(
                "all eligible credentials have exhausted their local RPM",
                retry_at,
            ))
        }
        GenerationSelection::RateLimited(_)
        | GenerationSelection::TemporarilyUnavailable(_)
        | GenerationSelection::RetryDeferred(_) => {
            wait_for_candidate(coordinator, policy, wait_state, try_select).await
        }
    }
}

pub(super) async fn wait_for_candidate(
    coordinator: &Arc<QueueCoordinator>,
    policy: QueuePolicy,
    wait_state: &SelectionWaitState,
    mut try_select: impl FnMut() -> Result<GenerationSelection, PublicError>,
) -> Result<SelectedCandidate, PublicError> {
    let Some(mut changes) = wait_state.queue_changes(coordinator, policy.max_waiting_requests())
    else {
        return Err(rate_limit_error("request queue is full"));
    };
    let deadline = wait_state.queue(policy.queue_timeout());

    loop {
        let _observed_epoch = *changes.borrow_and_update();
        let retry_at = match try_select()? {
            GenerationSelection::Acquired(selected) => return Ok(*selected),
            GenerationSelection::NoCandidates => return Err(no_available_credentials()),
            GenerationSelection::RateLimited(retry_at) => retry_at,
            GenerationSelection::TemporarilyUnavailable(retry_at) => Some(retry_at),
            GenerationSelection::RetryDeferred(retry_at) => Some(retry_at),
        };
        if Instant::now() >= deadline {
            return final_selection_or_timeout(&mut try_select);
        }
        if let Some(retry_at) = retry_at {
            let wake_at = retry_at.min(deadline);
            tokio::select! {
                changed = changes.changed() => {
                    if changed.is_err() {
                        return Err(crate::public_request::response::internal_error());
                    }
                }
                () = sleep_until(wake_at) => {}
            }
        } else {
            match timeout_at(deadline, changes.changed()).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => return Err(crate::public_request::response::internal_error()),
                Err(_) => return final_selection_or_timeout(&mut try_select),
            }
        }
    }
}

fn final_selection_or_timeout(
    try_select: &mut impl FnMut() -> Result<GenerationSelection, PublicError>,
) -> Result<SelectedCandidate, PublicError> {
    match try_select()? {
        GenerationSelection::Acquired(selected) => Ok(*selected),
        GenerationSelection::NoCandidates => Err(no_available_credentials()),
        GenerationSelection::TemporarilyUnavailable(retry_at) => {
            Err(temporarily_unavailable(retry_at))
        }
        GenerationSelection::RetryDeferred(retry_at) => Err(temporarily_unavailable(retry_at)),
        GenerationSelection::RateLimited(retry_at) => Err(rate_limited(
            "all eligible credentials have exhausted their local RPM",
            retry_at,
        )),
    }
}

fn earliest(current: Option<Instant>, candidate: Instant) -> Option<Instant> {
    Some(current.map_or(candidate, |current| current.min(candidate)))
}

fn earliest_optional(first: Option<Instant>, second: Option<Instant>) -> Option<Instant> {
    match second {
        Some(second) => earliest(first, second),
        None => first,
    }
}

#[cfg(test)]
pub(super) fn try_select_for_test(
    fallback_on_rate_limit: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<GenerationSelection, PublicError> {
    let mut filters = RequestFilterRecorder::default();
    try_select_for_test_with_recorder(fallback_on_rate_limit, tiers, &mut filters, tie_breaker)
}

#[cfg(test)]
pub(super) fn try_select_for_test_with_recorder(
    fallback_on_rate_limit: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    filters: &mut RequestFilterRecorder,
    tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<GenerationSelection, PublicError> {
    try_select_with(
        ReliabilityPolicy::from_settings(
            any2api_domain::SettingsConfiguration::defaults().reliability(),
        ),
        fallback_on_rate_limit,
        SelectionPool {
            tiers,
            selection_state: &CandidateSelectionState::default(),
            credential_eligible: &|_| true,
        },
        filters,
        tie_breaker,
        |_, _| true,
    )
}

#[cfg(test)]
pub(super) fn try_select_for_test_with_cursor(
    fallback_on_rate_limit: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    tie_breaker: impl FnMut(u16) -> Option<u64>,
    advance_cursor: impl FnMut(u16, u64) -> bool,
) -> Result<GenerationSelection, PublicError> {
    let mut filters = RequestFilterRecorder::default();
    try_select_with(
        ReliabilityPolicy::from_settings(
            any2api_domain::SettingsConfiguration::defaults().reliability(),
        ),
        fallback_on_rate_limit,
        SelectionPool {
            tiers,
            selection_state: &CandidateSelectionState::default(),
            credential_eligible: &|_| true,
        },
        &mut filters,
        tie_breaker,
        advance_cursor,
    )
}

#[cfg(test)]
pub(super) fn try_select_for_test_with_state(
    fallback_on_rate_limit: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    selection_state: &CandidateSelectionState,
    credential_eligible: &(dyn Fn(RoutingCredentialId) -> bool + Sync),
    tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<GenerationSelection, PublicError> {
    let mut filters = RequestFilterRecorder::default();
    try_select_with(
        ReliabilityPolicy::from_settings(
            any2api_domain::SettingsConfiguration::defaults().reliability(),
        ),
        fallback_on_rate_limit,
        SelectionPool {
            tiers,
            selection_state,
            credential_eligible,
        },
        &mut filters,
        tie_breaker,
        |_, _| true,
    )
}
