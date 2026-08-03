use std::time::Duration;

use tokio::time::{Instant, sleep_until, timeout_at};

use super::super::SelectedCandidate;
use super::{FixedSelectionError, filter_recorder::RequestFilterRecorder};
use crate::{
    configuration::PublishedSnapshot,
    credential::CredentialFilterKind,
    health::{HealthAcquireError, ReliabilityPolicy},
    routing::RouteCandidate,
};

pub(super) async fn select(
    snapshot: &PublishedSnapshot,
    candidate: &RouteCandidate,
    wait_timeout: Duration,
) -> Result<SelectedCandidate, FixedSelectionError> {
    let mut filters = RequestFilterRecorder::default();
    match try_selected(snapshot.reliability_policy(), candidate, &mut filters)? {
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
        let retry_at = match try_selected(snapshot.reliability_policy(), candidate, &mut filters)? {
            FixedAttempt::Acquired(selected) => return Ok(*selected),
            FixedAttempt::Waiting(retry_at) => retry_at,
        };
        if Instant::now() >= deadline {
            return final_selection(snapshot.reliability_policy(), candidate, &mut filters);
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
                Err(_) => {
                    return final_selection(snapshot.reliability_policy(), candidate, &mut filters);
                }
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
    filters: &mut RequestFilterRecorder,
) -> Result<FixedAttempt, FixedSelectionError> {
    try_selected_with(policy, candidate, filters, || {})
}

fn try_selected_with(
    policy: ReliabilityPolicy,
    candidate: &RouteCandidate,
    filters: &mut RequestFilterRecorder,
    after_reservation: impl FnOnce(),
) -> Result<FixedAttempt, FixedSelectionError> {
    match candidate.health_availability(&policy) {
        Ok(()) => {}
        Err(error) => {
            filters.record(candidate, error.kind());
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
            filters.record(candidate, CredentialFilterKind::RateLimit);
            return Ok(FixedAttempt::Waiting(rate_limited.retry_at));
        }
    };
    after_reservation();
    let (permit, health) = match candidate.acquire_health_with_rpm_reservation(policy, permit) {
        Ok(acquired) => acquired,
        Err(error) => {
            filters.record(candidate, error.kind());
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
    filters: &mut RequestFilterRecorder,
) -> Result<SelectedCandidate, FixedSelectionError> {
    match try_selected(policy, candidate, filters)? {
        FixedAttempt::Acquired(selected) => Ok(*selected),
        FixedAttempt::Waiting(_) => Err(FixedSelectionError::Timeout),
    }
}

#[cfg(test)]
pub(super) fn try_selected_for_test(
    policy: ReliabilityPolicy,
    candidate: &RouteCandidate,
    after_reservation: impl FnOnce(),
) -> Result<Option<SelectedCandidate>, FixedSelectionError> {
    let mut filters = RequestFilterRecorder::default();
    match try_selected_with(policy, candidate, &mut filters, after_reservation)? {
        FixedAttempt::Acquired(selected) => Ok(Some(*selected)),
        FixedAttempt::Waiting(_) => Ok(None),
    }
}

#[cfg(test)]
pub(super) fn try_selected_with_recorder_for_test(
    policy: ReliabilityPolicy,
    candidate: &RouteCandidate,
    filters: &mut RequestFilterRecorder,
) -> Result<Option<SelectedCandidate>, FixedSelectionError> {
    match try_selected(policy, candidate, filters)? {
        FixedAttempt::Acquired(selected) => Ok(Some(*selected)),
        FixedAttempt::Waiting(_) => Ok(None),
    }
}
