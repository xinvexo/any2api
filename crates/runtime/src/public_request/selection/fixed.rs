use std::time::Duration;

use tokio::time::{Instant, timeout};

use super::super::{RequestPermit, SelectedCandidate};
use super::FixedSelectionError;
use crate::{
    health::{HealthAcquireError, ReliabilityPolicy},
    published_snapshot::PublishedSnapshot,
    route_candidates::RouteCandidate,
};

pub(super) async fn select(
    snapshot: &PublishedSnapshot,
    candidate: &RouteCandidate,
    wait_timeout: Duration,
) -> Result<SelectedCandidate, FixedSelectionError> {
    if let Some(selected) = try_selected(snapshot.reliability_policy(), candidate)? {
        return Ok(selected);
    }
    let Some(ticket) = snapshot
        .queue_coordinator()
        .try_ticket(snapshot.queue_policy().max_waiting_requests())
    else {
        return Err(FixedSelectionError::QueueFull);
    };
    let mut changes = ticket.subscribe();
    let _fixed_waiter = candidate.binding.register_fixed_waiter();
    let started_at = Instant::now();

    loop {
        let _observed_epoch = *changes.borrow_and_update();
        if let Some(selected) = try_selected(snapshot.reliability_policy(), candidate)? {
            return Ok(selected);
        }
        let remaining = wait_timeout.saturating_sub(Instant::now().duration_since(started_at));
        if remaining.is_zero() {
            return try_selected(snapshot.reliability_policy(), candidate)?
                .ok_or(FixedSelectionError::Timeout);
        }
        match timeout(remaining, changes.changed()).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Err(FixedSelectionError::Internal),
            Err(_) => {
                return try_selected(snapshot.reliability_policy(), candidate)?
                    .ok_or(FixedSelectionError::Timeout);
            }
        }
    }
}

fn try_selected(
    policy: ReliabilityPolicy,
    candidate: &RouteCandidate,
) -> Result<Option<SelectedCandidate>, FixedSelectionError> {
    match candidate.health_availability(&policy) {
        Ok(()) => {}
        Err(error) => {
            candidate.record_health_filter(error);
            return match error.source() {
                HealthAcquireError::Temporary(_) => Ok(None),
                HealthAcquireError::Permanent => Err(FixedSelectionError::Unavailable),
            };
        }
    }
    let Some(permit) = candidate.binding.try_acquire_fixed() else {
        candidate.record_capacity_filter();
        return Ok(None);
    };
    let health = match candidate.acquire_health(policy) {
        Ok(health) => health,
        Err(error) => {
            candidate.record_health_filter(error);
            drop(permit);
            return match error.source() {
                HealthAcquireError::Temporary(_) => Ok(None),
                HealthAcquireError::Permanent => Err(FixedSelectionError::Unavailable),
            };
        }
    };
    candidate.record_generation_selection();
    Ok(Some(SelectedCandidate {
        candidate: candidate.clone(),
        permit: RequestPermit::Generation(permit),
        health,
    }))
}

#[cfg(test)]
pub(super) fn try_selected_for_test(
    policy: ReliabilityPolicy,
    candidate: &RouteCandidate,
) -> Result<Option<SelectedCandidate>, FixedSelectionError> {
    try_selected(policy, candidate)
}
