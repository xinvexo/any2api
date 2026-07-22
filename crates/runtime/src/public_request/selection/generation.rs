use std::{collections::BTreeMap, sync::Arc};

use any2api_domain::{FallbackTier, ModelRouteId, PublicError};
use tokio::time::{Instant, timeout};

use super::super::SelectedCandidate;
use super::{
    GenerationSelection, capacity_error, no_available_credentials, temporarily_unavailable,
};
use crate::{
    health::{HealthAcquireError, ReliabilityPolicy},
    published_snapshot::PublishedSnapshot,
    queue::{QueueCoordinator, QueuePolicy, SaturationAction},
    route_candidates::{CandidateExclusions, RouteCandidate},
    scheduler::{IndexedSelectAndAcquireResult, select_index_and_try_acquire},
};

pub(super) fn try_select(
    snapshot: &PublishedSnapshot,
    route_id: ModelRouteId,
    fallback_on_saturation: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    exclusions: &CandidateExclusions,
) -> Result<GenerationSelection, PublicError> {
    try_select_with(
        snapshot.reliability_policy(),
        fallback_on_saturation,
        tiers,
        exclusions,
        |tier| {
            snapshot
                .route_tier_cursor(route_id, FallbackTier::new(tier))
                .map(|cursor| cursor.reserve())
        },
    )
}

fn try_select_with(
    policy: ReliabilityPolicy,
    fallback_on_saturation: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    exclusions: &CandidateExclusions,
    mut tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<GenerationSelection, PublicError> {
    let mut saw_capacity = false;
    for (tier, candidates) in tiers {
        let mut retry_at = None;
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
                        candidate.record_health_filter(error);
                        match error.source() {
                            HealthAcquireError::Temporary(until) => {
                                retry_at = Some(
                                    retry_at.map_or(until, |current: Instant| current.min(until)),
                                );
                                false
                            }
                            HealthAcquireError::Permanent => false,
                        }
                    }
                }
            })
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            if let Some(retry_at) = retry_at {
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
            match select_index_and_try_acquire(&bindings, tie_breaker) {
                IndexedSelectAndAcquireResult::Acquired { index, permit } => {
                    let candidate = eligible[index].1;
                    let health = match candidate.acquire_health(policy) {
                        Ok(health) => health,
                        Err(error) => {
                            candidate.record_health_filter(error);
                            drop(permit);
                            if let HealthAcquireError::Temporary(until) = error.source() {
                                retry_at = Some(
                                    retry_at.map_or(until, |current: Instant| current.min(until)),
                                );
                            }
                            eligible.swap_remove(index);
                            continue;
                        }
                    };
                    candidate.record_generation_selection();
                    return Ok(GenerationSelection::Acquired(Box::new(SelectedCandidate {
                        candidate: candidate.clone(),
                        permit: super::super::RequestPermit::Generation(permit),
                        health,
                    })));
                }
                IndexedSelectAndAcquireResult::AtCapacity => {
                    for (_, candidate) in &eligible {
                        if candidate.binding.normal_capacity().is_full() {
                            candidate.record_capacity_filter();
                        }
                    }
                    saw_capacity = true;
                    if !fallback_on_saturation {
                        return Ok(GenerationSelection::AtCapacity);
                    }
                    break;
                }
                IndexedSelectAndAcquireResult::NoCandidates => break,
            }
        }
        if eligible.is_empty()
            && let Some(retry_at) = retry_at
        {
            return Ok(GenerationSelection::TemporarilyUnavailable(retry_at));
        }
    }
    Ok(if saw_capacity {
        GenerationSelection::AtCapacity
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
            if policy.on_saturated() == SaturationAction::Reject =>
        {
            Err(temporarily_unavailable(retry_at))
        }
        GenerationSelection::AtCapacity if policy.on_saturated() == SaturationAction::Reject => {
            Err(capacity_error("all eligible credentials are at capacity"))
        }
        GenerationSelection::AtCapacity | GenerationSelection::TemporarilyUnavailable(_) => {
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
        return Err(capacity_error("request queue is full"));
    };
    let mut changes = ticket.subscribe();
    let started_at = Instant::now();

    loop {
        let _observed_epoch = *changes.borrow_and_update();
        match try_select()? {
            GenerationSelection::Acquired(selected) => return Ok(*selected),
            GenerationSelection::NoCandidates => return Err(no_available_credentials()),
            GenerationSelection::AtCapacity | GenerationSelection::TemporarilyUnavailable(_) => {}
        }
        let elapsed = Instant::now().saturating_duration_since(started_at);
        let remaining = policy.queue_timeout().saturating_sub(elapsed);
        if remaining.is_zero() {
            return final_selection_or_timeout(&mut try_select);
        }
        match timeout(remaining, changes.changed()).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Err(crate::public_request::response::internal_error()),
            Err(_) => return final_selection_or_timeout(&mut try_select),
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
        GenerationSelection::AtCapacity => {
            Err(capacity_error("all eligible credentials are at capacity"))
        }
    }
}

#[cfg(test)]
pub(super) fn try_select_for_test(
    fallback_on_saturation: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<GenerationSelection, PublicError> {
    try_select_with(
        ReliabilityPolicy::from_settings(
            any2api_domain::SettingsConfiguration::defaults().reliability(),
        ),
        fallback_on_saturation,
        tiers,
        &CandidateExclusions::default(),
        tie_breaker,
    )
}
