use std::{sync::Arc, time::Duration};

use tokio::time::{Instant, sleep_until};

use super::super::SelectedCandidate;
use super::{FixedSelectionError, SelectionWaitState, filter_recorder::RequestFilterRecorder};
use crate::{
    configuration::PublishedSnapshot,
    credential::CredentialFilterKind,
    health::{HealthAcquireError, ReliabilityPolicy},
    routing::{QueueCoordinator, RouteCandidate},
};

pub(super) async fn select(
    snapshot: &PublishedSnapshot,
    candidate: &RouteCandidate,
    wait_timeout: Duration,
    wait_state: &SelectionWaitState,
) -> Result<SelectedCandidate, FixedSelectionError> {
    select_with_queue(
        snapshot.queue_coordinator(),
        snapshot.queue_policy().max_waiting_requests(),
        snapshot.reliability_policy(),
        candidate,
        wait_timeout,
        wait_state,
    )
    .await
}

async fn select_with_queue(
    queue: &Arc<QueueCoordinator>,
    max_waiting_requests: u32,
    policy: ReliabilityPolicy,
    candidate: &RouteCandidate,
    wait_timeout: Duration,
    wait_state: &SelectionWaitState,
) -> Result<SelectedCandidate, FixedSelectionError> {
    let mut filters = RequestFilterRecorder::default();
    match try_selected(policy, candidate, &mut filters)? {
        FixedAttempt::Acquired(selected) => return Ok(*selected),
        FixedAttempt::Waiting(_) => {}
    }
    let Some(mut changes) = wait_state.queue_changes(queue, max_waiting_requests) else {
        return Err(FixedSelectionError::QueueFull);
    };
    let deadline = wait_state.binding(wait_timeout);
    let mut credential_changes = candidate.binding.subscribe_changes();
    let mut fixed_waiter = None;

    loop {
        let _observed_epoch = *changes.borrow_and_update();
        let _observed_credential = *credential_changes.borrow_and_update();
        let waiting = match try_selected(policy, candidate, &mut filters)? {
            FixedAttempt::Acquired(selected) => return Ok(*selected),
            FixedAttempt::Waiting(waiting) => waiting,
        };
        if waiting.reserves_rpm_slot() {
            if fixed_waiter.is_none() {
                fixed_waiter = Some(candidate.binding.register_fixed_waiter());
            }
        } else {
            fixed_waiter = None;
        }
        if Instant::now() >= deadline {
            return final_selection(policy, candidate, &mut filters);
        }
        let retry_at = waiting.retry_at();
        let wake_at = retry_at.map_or(deadline, |retry_at| retry_at.min(deadline));
        tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() {
                    return Err(FixedSelectionError::Internal);
                }
            }
            changed = credential_changes.changed() => {
                if changed.is_err() {
                    return Err(FixedSelectionError::Internal);
                }
            }
            () = sleep_until(wake_at) => {
                if retry_at.is_none() {
                    return final_selection(policy, candidate, &mut filters);
                }
            }
        }
    }
}

enum FixedAttempt {
    Acquired(Box<SelectedCandidate>),
    Waiting(FixedWait),
}

#[derive(Clone, Copy)]
enum FixedWait {
    Health(Instant),
    RateLimit(Option<Instant>),
}

impl FixedWait {
    const fn retry_at(self) -> Option<Instant> {
        match self {
            Self::Health(retry_at) => Some(retry_at),
            Self::RateLimit(retry_at) => retry_at,
        }
    }

    const fn reserves_rpm_slot(self) -> bool {
        matches!(self, Self::RateLimit(_))
    }
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
    if !candidate.admission_active() {
        return Err(FixedSelectionError::Unavailable);
    }
    match candidate.health_availability(&policy) {
        Ok(()) => {}
        Err(error) => {
            filters.record(candidate, error.kind());
            return match error.source() {
                HealthAcquireError::Temporary(unavailability) => Ok(FixedAttempt::Waiting(
                    FixedWait::Health(unavailability.until()),
                )),
                HealthAcquireError::Permanent => Err(FixedSelectionError::Unavailable),
            };
        }
    }
    let permit = match candidate.binding.try_reserve_fixed() {
        Ok(permit) => permit,
        Err(rate_limited) => {
            filters.record(candidate, CredentialFilterKind::RateLimit);
            return Ok(FixedAttempt::Waiting(FixedWait::RateLimit(
                rate_limited.retry_at,
            )));
        }
    };
    after_reservation();
    let (permit, health) = match candidate.acquire_health_with_rpm_reservation(policy, permit) {
        Ok(acquired) => acquired,
        Err(error) => {
            filters.record(candidate, error.kind());
            return match error.source() {
                HealthAcquireError::Temporary(unavailability) => Ok(FixedAttempt::Waiting(
                    FixedWait::Health(unavailability.until()),
                )),
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

#[cfg(test)]
pub(super) async fn select_with_queue_for_test(
    queue: &Arc<QueueCoordinator>,
    max_waiting_requests: u32,
    policy: ReliabilityPolicy,
    candidate: &RouteCandidate,
    wait_timeout: Duration,
) -> Result<SelectedCandidate, FixedSelectionError> {
    let wait_state = SelectionWaitState::default();
    select_with_queue(
        queue,
        max_waiting_requests,
        policy,
        candidate,
        wait_timeout,
        &wait_state,
    )
    .await
}
