use std::time::Duration;

use tokio::time::{Instant, timeout};

use super::super::{RequestPermit, SelectedCandidate};
use super::FixedSelectionError;
use crate::{
    health::HealthAcquireError, published_snapshot::PublishedSnapshot,
    route_candidates::RouteCandidate,
};

pub(super) async fn select(
    snapshot: &PublishedSnapshot,
    candidate: &RouteCandidate,
    wait_timeout: Duration,
) -> Result<SelectedCandidate, FixedSelectionError> {
    if let Some(selected) = try_selected(snapshot, candidate)? {
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
        if let Some(selected) = try_selected(snapshot, candidate)? {
            return Ok(selected);
        }
        let remaining = wait_timeout.saturating_sub(Instant::now().duration_since(started_at));
        if remaining.is_zero() {
            return try_selected(snapshot, candidate)?.ok_or(FixedSelectionError::Timeout);
        }
        match timeout(remaining, changes.changed()).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Err(FixedSelectionError::Internal),
            Err(_) => {
                return try_selected(snapshot, candidate)?.ok_or(FixedSelectionError::Timeout);
            }
        }
    }
}

fn try_selected(
    snapshot: &PublishedSnapshot,
    candidate: &RouteCandidate,
) -> Result<Option<SelectedCandidate>, FixedSelectionError> {
    match candidate.health_availability(&snapshot.reliability_policy()) {
        Ok(()) => {}
        Err(HealthAcquireError::Temporary(_)) => return Ok(None),
        Err(HealthAcquireError::Permanent) => return Err(FixedSelectionError::Unavailable),
    }
    let Some(permit) = candidate.binding.try_acquire_fixed() else {
        return Ok(None);
    };
    let health = match candidate.acquire_health(snapshot.reliability_policy()) {
        Ok(health) => health,
        Err(HealthAcquireError::Temporary(_)) => {
            drop(permit);
            return Ok(None);
        }
        Err(HealthAcquireError::Permanent) => {
            drop(permit);
            return Err(FixedSelectionError::Unavailable);
        }
    };
    Ok(Some(SelectedCandidate {
        candidate: candidate.clone(),
        permit: RequestPermit::Generation(permit),
        health,
    }))
}
