use std::{collections::BTreeMap, sync::Arc};

use any2api_domain::{FallbackTier, ModelRouteId, ProtocolOperation, PublicError, PublicErrorCode};
use tokio::time::{Instant, timeout};

use super::{
    RequestPermit, SelectedCandidate,
    response::{internal_error, public_error},
};
use crate::{
    auxiliary_scheduler::{AuxiliaryScheduler, AuxiliarySelectAndAcquireResult},
    published_snapshot::PublishedSnapshot,
    queue::{QueueCoordinator, QueuePolicy, SaturationAction},
    route_candidates::RouteCandidate,
    scheduler::{IndexedSelectAndAcquireResult, select_index_and_try_acquire},
};

pub(super) async fn select_candidate(
    snapshot: &PublishedSnapshot,
    operation: ProtocolOperation,
    route_id: ModelRouteId,
    fallback_on_saturation: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
) -> Result<SelectedCandidate, PublicError> {
    if operation == ProtocolOperation::MessagesCountTokens {
        return select_auxiliary_candidate(snapshot, route_id, tiers);
    }

    let try_select =
        || try_select_generation_candidate(snapshot, route_id, fallback_on_saturation, tiers);
    select_generation_candidate(
        snapshot.queue_coordinator(),
        snapshot.queue_policy(),
        try_select,
    )
    .await
}

enum GenerationSelection {
    Acquired(SelectedCandidate),
    AtCapacity,
    NoCandidates,
}

fn try_select_generation_candidate(
    snapshot: &PublishedSnapshot,
    route_id: ModelRouteId,
    fallback_on_saturation: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
) -> Result<GenerationSelection, PublicError> {
    try_select_generation_candidate_with(fallback_on_saturation, tiers, |tier| {
        snapshot
            .route_tier_cursor(route_id, FallbackTier::new(tier))
            .map(|cursor| cursor.reserve())
    })
}

fn try_select_generation_candidate_with(
    fallback_on_saturation: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    mut tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<GenerationSelection, PublicError> {
    let mut saw_capacity = false;
    for (tier, candidates) in tiers {
        let bindings = candidates
            .iter()
            .map(|candidate| candidate.binding.clone())
            .collect::<Vec<_>>();
        let tie_breaker = tie_breaker(*tier).ok_or_else(internal_error)?;
        match select_index_and_try_acquire(&bindings, tie_breaker) {
            IndexedSelectAndAcquireResult::Acquired { index, permit } => {
                return Ok(GenerationSelection::Acquired(SelectedCandidate {
                    candidate: candidates[index].clone(),
                    permit: RequestPermit::Generation(permit),
                }));
            }
            IndexedSelectAndAcquireResult::AtCapacity => {
                saw_capacity = true;
                if !fallback_on_saturation {
                    return Ok(GenerationSelection::AtCapacity);
                }
            }
            IndexedSelectAndAcquireResult::NoCandidates => {}
        }
    }
    Ok(if saw_capacity {
        GenerationSelection::AtCapacity
    } else {
        GenerationSelection::NoCandidates
    })
}

async fn select_generation_candidate(
    coordinator: &Arc<QueueCoordinator>,
    policy: QueuePolicy,
    mut try_select: impl FnMut() -> Result<GenerationSelection, PublicError>,
) -> Result<SelectedCandidate, PublicError> {
    match try_select()? {
        GenerationSelection::Acquired(selected) => Ok(selected),
        GenerationSelection::NoCandidates => Err(no_available_credentials()),
        GenerationSelection::AtCapacity if policy.on_saturated() == SaturationAction::Reject => {
            Err(capacity_error("all eligible credentials are at capacity"))
        }
        GenerationSelection::AtCapacity => {
            wait_for_generation_candidate(coordinator, policy, try_select).await
        }
    }
}

async fn wait_for_generation_candidate(
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
            GenerationSelection::Acquired(selected) => return Ok(selected),
            GenerationSelection::NoCandidates => return Err(no_available_credentials()),
            GenerationSelection::AtCapacity => {}
        }

        let elapsed = Instant::now().saturating_duration_since(started_at);
        let remaining = policy.queue_timeout().saturating_sub(elapsed);
        if remaining.is_zero() {
            return final_selection_or_timeout(&mut try_select);
        }
        match timeout(remaining, changes.changed()).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Err(internal_error()),
            Err(_) => return final_selection_or_timeout(&mut try_select),
        }
    }
}

fn final_selection_or_timeout(
    try_select: &mut impl FnMut() -> Result<GenerationSelection, PublicError>,
) -> Result<SelectedCandidate, PublicError> {
    match try_select()? {
        GenerationSelection::Acquired(selected) => Ok(selected),
        GenerationSelection::NoCandidates => Err(no_available_credentials()),
        GenerationSelection::AtCapacity => {
            Err(capacity_error("all eligible credentials are at capacity"))
        }
    }
}

fn select_auxiliary_candidate(
    snapshot: &PublishedSnapshot,
    route_id: ModelRouteId,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
) -> Result<SelectedCandidate, PublicError> {
    select_auxiliary_candidate_with(snapshot.auxiliary_scheduler(), tiers, |tier| {
        snapshot
            .route_tier_cursor(route_id, FallbackTier::new(tier))
            .map(|cursor| cursor.reserve())
    })
}

fn select_auxiliary_candidate_with(
    scheduler: &Arc<AuxiliaryScheduler>,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    mut tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<SelectedCandidate, PublicError> {
    for (tier, candidates) in tiers {
        let bindings = candidates
            .iter()
            .map(|candidate| candidate.binding.clone())
            .collect::<Vec<_>>();
        let tie_breaker = tie_breaker(*tier).ok_or_else(internal_error)?;
        match scheduler.select_index_and_try_acquire(&bindings, tie_breaker) {
            AuxiliarySelectAndAcquireResult::Acquired { index, permit } => {
                return Ok(SelectedCandidate {
                    candidate: candidates[index].clone(),
                    permit: RequestPermit::Auxiliary(permit),
                });
            }
            AuxiliarySelectAndAcquireResult::AtCapacity => {
                return Err(capacity_error("auxiliary request capacity is full"));
            }
            AuxiliarySelectAndAcquireResult::NoCandidates => {}
        }
    }
    Err(no_available_credentials())
}

fn capacity_error(message: &'static str) -> PublicError {
    public_error(PublicErrorCode::LocalConcurrencyLimit, message)
}

fn no_available_credentials() -> PublicError {
    public_error(
        PublicErrorCode::NoAvailableCredential,
        "no eligible provider credential is available",
    )
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
