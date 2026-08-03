use std::{collections::BTreeMap, sync::Arc};

use any2api_domain::{FallbackTier, ModelRouteId, PublicError};
use tokio::time::{Instant, sleep_until, timeout_at};

use super::super::SelectedCandidate;
use super::{
    GenerationSelection, filter_recorder::RequestFilterRecorder, no_available_credentials,
    rate_limit_error, rate_limited, temporarily_unavailable,
};
use crate::{
    configuration::PublishedSnapshot,
    credential::CredentialFilterKind,
    health::{HealthAcquireError, ReliabilityPolicy},
    routing::{
        CandidateExclusions, IndexedSelectAndReserveResult, QueueCoordinator, QueuePolicy,
        RateLimitAction, RouteCandidate, select_index_and_try_reserve,
    },
};

pub(super) fn try_select(
    snapshot: &PublishedSnapshot,
    route_id: ModelRouteId,
    fallback_on_rate_limit: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    exclusions: &CandidateExclusions,
    filters: &mut RequestFilterRecorder,
) -> Result<GenerationSelection, PublicError> {
    try_select_with(
        snapshot.reliability_policy(),
        fallback_on_rate_limit,
        tiers,
        exclusions,
        filters,
        |tier| {
            snapshot
                .route_tier_cursor(route_id, FallbackTier::new(tier))
                .map(|cursor| cursor.reserve())
        },
    )
}

fn try_select_with(
    policy: ReliabilityPolicy,
    fallback_on_rate_limit: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    exclusions: &CandidateExclusions,
    filters: &mut RequestFilterRecorder,
    mut tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<GenerationSelection, PublicError> {
    let mut saw_rate_limit = false;
    let mut rate_retry_at = None;
    for (tier, candidates) in tiers {
        let mut health_retry_at = None;
        let eligible = candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                if !exclusions.allows(candidate) {
                    return false;
                }
                match candidate.health_availability(&policy) {
                    Ok(()) => true,
                    Err(error) => {
                        filters.record(candidate, error.kind());
                        if let HealthAcquireError::Temporary(until) = error.source() {
                            health_retry_at = earliest(health_retry_at, until);
                        }
                        false
                    }
                }
            })
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            if let Some(retry_at) = health_retry_at {
                return Ok(GenerationSelection::TemporarilyUnavailable(retry_at));
            }
            continue;
        }
        let mut eligible = eligible;
        while !eligible.is_empty() {
            let bindings = eligible
                .iter()
                .map(|(_, candidate)| candidate.binding.clone())
                .collect::<Vec<_>>();
            let tie_breaker =
                tie_breaker(*tier).ok_or_else(crate::public_request::response::internal_error)?;
            match select_index_and_try_reserve(&bindings, tie_breaker) {
                IndexedSelectAndReserveResult::Reserved { index, permit } => {
                    let candidate = eligible[index].1;
                    let (permit, health) =
                        match candidate.acquire_health_with_rpm_reservation(policy, permit) {
                            Ok(acquired) => acquired,
                            Err(error) => {
                                filters.record(candidate, error.kind());
                                if let HealthAcquireError::Temporary(until) = error.source() {
                                    health_retry_at = earliest(health_retry_at, until);
                                }
                                eligible.swap_remove(index);
                                continue;
                            }
                        };
                    candidate.record_selection();
                    return Ok(GenerationSelection::Acquired(Box::new(SelectedCandidate {
                        candidate: candidate.clone(),
                        permit,
                        health,
                    })));
                }
                IndexedSelectAndReserveResult::RateLimited { retry_at } => {
                    for (_, candidate) in &eligible {
                        filters.record(candidate, CredentialFilterKind::RateLimit);
                    }
                    saw_rate_limit = true;
                    if let Some(retry_at) = retry_at {
                        rate_retry_at = earliest(rate_retry_at, retry_at);
                    }
                    if !fallback_on_rate_limit {
                        return Ok(GenerationSelection::RateLimited(rate_retry_at));
                    }
                    break;
                }
            }
        }
        if eligible.is_empty()
            && let Some(retry_at) = health_retry_at
        {
            return Ok(GenerationSelection::TemporarilyUnavailable(retry_at));
        }
    }
    Ok(if saw_rate_limit {
        GenerationSelection::RateLimited(rate_retry_at)
    } else {
        GenerationSelection::NoCandidates
    })
}

pub(super) async fn select_with_queue(
    coordinator: &Arc<QueueCoordinator>,
    policy: QueuePolicy,
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
        GenerationSelection::RateLimited(_) | GenerationSelection::TemporarilyUnavailable(_) => {
            wait_for_candidate(coordinator, policy, try_select).await
        }
    }
}

pub(super) async fn wait_for_candidate(
    coordinator: &Arc<QueueCoordinator>,
    policy: QueuePolicy,
    mut try_select: impl FnMut() -> Result<GenerationSelection, PublicError>,
) -> Result<SelectedCandidate, PublicError> {
    let Some(ticket) = coordinator.try_ticket(policy.max_waiting_requests()) else {
        return Err(rate_limit_error("request queue is full"));
    };
    let mut changes = ticket.subscribe();
    let deadline = Instant::now() + policy.queue_timeout();

    loop {
        let _observed_epoch = *changes.borrow_and_update();
        let retry_at = match try_select()? {
            GenerationSelection::Acquired(selected) => return Ok(*selected),
            GenerationSelection::NoCandidates => return Err(no_available_credentials()),
            GenerationSelection::RateLimited(retry_at) => retry_at,
            GenerationSelection::TemporarilyUnavailable(retry_at) => Some(retry_at),
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
        GenerationSelection::RateLimited(retry_at) => Err(rate_limited(
            "all eligible credentials have exhausted their local RPM",
            retry_at,
        )),
    }
}

fn earliest(current: Option<Instant>, candidate: Instant) -> Option<Instant> {
    Some(current.map_or(candidate, |current| current.min(candidate)))
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
        tiers,
        &CandidateExclusions::default(),
        filters,
        tie_breaker,
    )
}
