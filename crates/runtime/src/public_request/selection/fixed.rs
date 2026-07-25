use std::time::Duration;

use tokio::time::{Instant, sleep_until, timeout_at};

use super::super::SelectedCandidate;
use super::FixedSelectionError;
use crate::{
    configuration::PublishedSnapshot,
    health::{HealthAcquireError, ReliabilityPolicy},
    routing::RouteCandidate,
};

pub(super) async fn select(
    snapshot: &PublishedSnapshot,
    candidate: &RouteCandidate,
    wait_timeout: Duration,
) -> Result<SelectedCandidate, FixedSelectionError> {
    match try_selected(snapshot.reliability_policy(), candidate)? {
        FixedAttempt::Acquired(selected) => return Ok(*selected),
        FixedAttempt::Waiting(_) => {}
    }
    let Some(ticket) = snapshot
        .queue_coordinator()
        .try_ticket(snapshot.queue_policy().max_waiting_requests())
    else {
        return Err(FixedSelectionError::QueueFull);
    };
    let mut changes = ticket.subscribe();
    let _fixed_waiter = candidate.binding.register_fixed_waiter();
    let deadline = Instant::now() + wait_timeout;

    loop {
        let _observed_epoch = *changes.borrow_and_update();
        let retry_at = match try_selected(snapshot.reliability_policy(), candidate)? {
            FixedAttempt::Acquired(selected) => return Ok(*selected),
            FixedAttempt::Waiting(retry_at) => retry_at,
        };
        if Instant::now() >= deadline {
            return final_selection(snapshot.reliability_policy(), candidate);
        }
        if let Some(retry_at) = retry_at {
            let wake_at = retry_at.min(deadline);
            tokio::select! {
                changed = changes.changed() => {
                    if changed.is_err() {
                        return Err(FixedSelectionError::Internal);
                    }
                }
                () = sleep_until(wake_at) => {}
            }
        } else {
            match timeout_at(deadline, changes.changed()).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => return Err(FixedSelectionError::Internal),
                Err(_) => return final_selection(snapshot.reliability_policy(), candidate),
            }
        }
    }
}

enum FixedAttempt {
    Acquired(Box<SelectedCandidate>),
    Waiting(Option<Instant>),
}

fn try_selected(
    policy: ReliabilityPolicy,
    candidate: &RouteCandidate,
) -> Result<FixedAttempt, FixedSelectionError> {
    match candidate.health_availability(&policy) {
        Ok(()) => {}
        Err(error) => {
            candidate.record_health_filter(error);
            return match error.source() {
                HealthAcquireError::Temporary(retry_at) => {
                    Ok(FixedAttempt::Waiting(Some(retry_at)))
                }
                HealthAcquireError::Permanent => Err(FixedSelectionError::Unavailable),
            };
        }
    }
    let permit = match candidate.binding.try_reserve_fixed() {
        Ok(permit) => permit,
        Err(rate_limited) => {
            candidate.record_rate_limit_filter();
            return Ok(FixedAttempt::Waiting(rate_limited.retry_at));
        }
    };
    let health = match candidate.acquire_health(policy) {
        Ok(health) => health,
        Err(error) => {
            candidate.record_health_filter(error);
            drop(permit);
            return match error.source() {
                HealthAcquireError::Temporary(retry_at) => {
                    Ok(FixedAttempt::Waiting(Some(retry_at)))
                }
                HealthAcquireError::Permanent => Err(FixedSelectionError::Unavailable),
            };
        }
    };
    candidate.record_selection();
    Ok(FixedAttempt::Acquired(Box::new(SelectedCandidate {
        candidate: candidate.clone(),
        permit,
        health,
    })))
}

fn final_selection(
    policy: ReliabilityPolicy,
    candidate: &RouteCandidate,
) -> Result<SelectedCandidate, FixedSelectionError> {
    match try_selected(policy, candidate)? {
        FixedAttempt::Acquired(selected) => Ok(*selected),
        FixedAttempt::Waiting(_) => Err(FixedSelectionError::Timeout),
    }
}

#[cfg(test)]
pub(super) fn try_selected_for_test(
    policy: ReliabilityPolicy,
    candidate: &RouteCandidate,
) -> Result<Option<SelectedCandidate>, FixedSelectionError> {
    match try_selected(policy, candidate)? {
        FixedAttempt::Acquired(selected) => Ok(Some(*selected)),
        FixedAttempt::Waiting(_) => Ok(None),
    }
}
