use std::{collections::BTreeMap, sync::Arc};

use any2api_domain::{FallbackTier, ModelRouteId, PublicError};
use tokio::time::{Instant, sleep_until, timeout_at};

use super::super::SelectedCandidate;
use super::{
    GenerationSelection,
    filter_recorder::RequestFilterRecorder,
    no_available_credentials, rate_limit_error, rate_limited, temporarily_unavailable,
    tier::{self, TierScan},
};
use crate::{
    configuration::PublishedSnapshot,
    health::ReliabilityPolicy,
    routing::{
        CandidateExclusions, QueueCoordinator, QueuePolicy, RateLimitAction, RouteCandidate,
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
        |tier, skipped| {
            snapshot
                .route_tier_cursor(route_id, FallbackTier::new(tier))
                .is_some_and(|cursor| {
                    cursor.advance_by(skipped);
                    true
                })
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
    mut advance_cursor: impl FnMut(u16, u64) -> bool,
) -> Result<GenerationSelection, PublicError> {
    let mut saw_rate_limit = false;
    let mut rate_retry_at = None;
    let mut skipped_retry_at = None;
    for (tier, candidates) in tiers {
        let tie_breaker =
            tie_breaker(*tier).ok_or_else(crate::public_request::response::internal_error)?;
        match tier::scan(policy, candidates, exclusions, filters, tie_breaker) {
            TierScan::Acquired { selected, skipped } => {
                if !advance_cursor(*tier, skipped) {
                    return Err(crate::public_request::response::internal_error());
                }
                return Ok(GenerationSelection::Acquired(selected));
            }
            TierScan::RateLimited { retry_at } => {
                saw_rate_limit = true;
                if let Some(retry_at) = retry_at {
                    rate_retry_at = earliest(rate_retry_at, retry_at);
                }
                if !fallback_on_rate_limit {
                    return Ok(GenerationSelection::RateLimited(rate_retry_at));
                }
            }
            TierScan::Exhausted {
                outage_retry_at,
                cooldown_retry_at,
            } => {
                // The whole tier is temporarily blocked. Upstream rate-limit
                // and quota cooldowns keep wait-in-place semantics unless the
                // route explicitly spills them to a lower tier.
                if let Some(retry_at) = cooldown_retry_at
                    && !fallback_on_rate_limit
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
    Ok(if saw_rate_limit {
        GenerationSelection::RateLimited(rate_retry_at)
    } else if let Some(retry_at) = skipped_retry_at {
        GenerationSelection::TemporarilyUnavailable(retry_at)
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
        tiers,
        &CandidateExclusions::default(),
        &mut filters,
        tie_breaker,
        advance_cursor,
    )
}
