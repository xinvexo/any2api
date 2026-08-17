use std::time::Duration;

use any2api_domain::PublicError;
use tokio::{
    sync::watch,
    time::{Instant, sleep_until, timeout_at},
};

use super::{
    AffinitySelection, AffinitySelectionInput, affinity_error, finish_unbound, select_bound,
};
use crate::{
    affinity::{BindingCreationPhase, BindingLease, BindingStart},
    public_request::selection::{
        CandidateSelector, GenerationSelection, no_available_credentials, rate_limit_error,
        rate_limited, temporarily_unavailable,
    },
    routing::{QueueCoordinator, QueueTicket, RateLimitAction},
};

pub(super) async fn select_session(
    input: &AffinitySelectionInput<'_>,
    raw: &str,
) -> Result<AffinitySelection, PublicError> {
    let snapshot = input.snapshot;
    let affinity_policy = snapshot.affinity_policy();
    let queue_policy = snapshot.queue_policy();
    let mut selector = CandidateSelector::new(
        snapshot,
        input.route_id,
        input.fallback_on_rate_limit,
        input.tiers,
        input.selection_state,
        input.credential_eligible,
    );
    let mut lease = None;
    let mut wait = None;
    let mut scheduler_deadline = None;
    let mut attempting_deadline = None;
    let mut waiting_on_attempt = false;

    loop {
        if waiting_on_attempt
            && attempting_deadline.is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(binding_wait_timeout());
        }
        waiting_on_attempt = false;
        let observed_epoch = observe_epoch(&mut wait)
            .unwrap_or_else(|| snapshot.queue_coordinator().current_epoch());
        if lease.is_none() {
            match snapshot
                .affinity_registry()
                .begin_session(input.dialect, input.route_id, raw, affinity_policy.ttl())
                .map_err(affinity_error)?
            {
                BindingStart::Create(created) => lease = Some(created),
                BindingStart::Wait(BindingCreationPhase::Selecting) => {
                    let deadline =
                        absolute_deadline(&mut scheduler_deadline, queue_policy.queue_timeout());
                    if Instant::now() >= deadline {
                        return Err(selection_coordination_timeout());
                    }
                    if ensure_queue_wait(
                        &mut wait,
                        snapshot.queue_coordinator(),
                        queue_policy.max_waiting_requests(),
                    )? {
                        continue;
                    }
                    wait_for_epoch(wait.as_mut().expect("queue wait exists"), deadline).await?;
                    continue;
                }
                BindingStart::Wait(BindingCreationPhase::Attempting) => {
                    let deadline =
                        absolute_deadline(&mut attempting_deadline, affinity_policy.wait_timeout());
                    if Instant::now() >= deadline {
                        return Err(binding_wait_timeout());
                    }
                    if ensure_queue_wait(
                        &mut wait,
                        snapshot.queue_coordinator(),
                        queue_policy.max_waiting_requests(),
                    )? {
                        continue;
                    }
                    waiting_on_attempt = true;
                    wait_for_attempt(wait.as_mut().expect("queue wait exists"), deadline).await?;
                    continue;
                }
                BindingStart::Bound(binding) => {
                    drop(wait.take());
                    return select_bound(input, binding.target().clone(), None).await;
                }
            }
        }

        match selector.try_select()? {
            GenerationSelection::Acquired(selected) => {
                let selected = *selected;
                let current = lease.as_mut().expect("selection requires a creating lease");
                if let Err(error) = current.mark_attempting() {
                    selected.rollback_before_attempt();
                    return Err(affinity_error(error));
                }
                return Ok(finish_unbound(input, selected, lease.take()));
            }
            GenerationSelection::NoCandidates => return Err(no_available_credentials()),
            GenerationSelection::RateLimited(retry_at) => {
                let error = rate_limited(
                    "all eligible credentials have exhausted their local RPM",
                    retry_at,
                );
                if queue_policy.on_rate_limited() == RateLimitAction::Reject {
                    return Err(error);
                }
                let released_epoch = release_selecting(&mut lease)?;
                let deadline =
                    absolute_deadline(&mut scheduler_deadline, queue_policy.queue_timeout());
                if Instant::now() >= deadline {
                    return Err(error);
                }
                ensure_queue_wait(
                    &mut wait,
                    snapshot.queue_coordinator(),
                    queue_policy.max_waiting_requests(),
                )?;
                if release_observed_external_change(
                    wait.as_mut().expect("queue wait exists"),
                    observed_epoch,
                    released_epoch,
                ) {
                    continue;
                }
                wait_for_candidate(
                    wait.as_mut().expect("queue wait exists"),
                    deadline,
                    retry_at,
                )
                .await?;
            }
            GenerationSelection::TemporarilyUnavailable(retry_at) => {
                let error = temporarily_unavailable(retry_at);
                if queue_policy.on_rate_limited() == RateLimitAction::Reject {
                    return Err(error);
                }
                let released_epoch = release_selecting(&mut lease)?;
                let deadline =
                    absolute_deadline(&mut scheduler_deadline, queue_policy.queue_timeout());
                if Instant::now() >= deadline {
                    return Err(error);
                }
                ensure_queue_wait(
                    &mut wait,
                    snapshot.queue_coordinator(),
                    queue_policy.max_waiting_requests(),
                )?;
                if release_observed_external_change(
                    wait.as_mut().expect("queue wait exists"),
                    observed_epoch,
                    released_epoch,
                ) {
                    continue;
                }
                wait_for_candidate(
                    wait.as_mut().expect("queue wait exists"),
                    deadline,
                    Some(retry_at),
                )
                .await?;
            }
            GenerationSelection::RetryDeferred(retry_at) => {
                let error = temporarily_unavailable(retry_at);
                let released_epoch = release_selecting(&mut lease)?;
                let deadline =
                    absolute_deadline(&mut scheduler_deadline, queue_policy.queue_timeout());
                if Instant::now() >= deadline {
                    return Err(error);
                }
                ensure_queue_wait(
                    &mut wait,
                    snapshot.queue_coordinator(),
                    queue_policy.max_waiting_requests(),
                )?;
                if release_observed_external_change(
                    wait.as_mut().expect("queue wait exists"),
                    observed_epoch,
                    released_epoch,
                ) {
                    continue;
                }
                wait_for_candidate(
                    wait.as_mut().expect("queue wait exists"),
                    deadline,
                    Some(retry_at),
                )
                .await?;
            }
        }
    }
}

struct SessionQueueWait {
    _ticket: QueueTicket,
    changes: watch::Receiver<u64>,
}

fn ensure_queue_wait(
    wait: &mut Option<SessionQueueWait>,
    queue: &std::sync::Arc<QueueCoordinator>,
    max_waiting_requests: u32,
) -> Result<bool, PublicError> {
    if wait.is_some() {
        return Ok(false);
    }
    let ticket = queue
        .try_ticket(max_waiting_requests)
        .ok_or_else(|| rate_limit_error("request queue is full"))?;
    let changes = ticket.subscribe();
    *wait = Some(SessionQueueWait {
        _ticket: ticket,
        changes,
    });
    Ok(true)
}

fn observe_epoch(wait: &mut Option<SessionQueueWait>) -> Option<u64> {
    if let Some(wait) = wait {
        return Some(*wait.changes.borrow_and_update());
    }
    None
}

fn release_selecting(lease: &mut Option<BindingLease>) -> Result<u64, PublicError> {
    lease
        .take()
        .expect("candidate wait requires a selecting lease")
        .release_for_wait()
        .map_err(affinity_error)
}

fn release_observed_external_change(
    wait: &mut SessionQueueWait,
    observed_epoch: u64,
    released_epoch: u64,
) -> bool {
    let current_epoch = *wait.changes.borrow_and_update();
    observed_epoch.checked_add(1) != Some(released_epoch) || current_epoch > released_epoch
}

fn absolute_deadline(slot: &mut Option<Instant>, timeout: Duration) -> Instant {
    *slot.get_or_insert_with(|| Instant::now() + timeout)
}

async fn wait_for_epoch(wait: &mut SessionQueueWait, deadline: Instant) -> Result<(), PublicError> {
    match timeout_at(deadline, wait.changes.changed()).await {
        Ok(Ok(())) | Err(_) => Ok(()),
        Ok(Err(_)) => Err(super::internal_error()),
    }
}

async fn wait_for_attempt(
    wait: &mut SessionQueueWait,
    deadline: Instant,
) -> Result<(), PublicError> {
    match timeout_at(deadline, wait.changes.changed()).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err(super::internal_error()),
        Err(_) => Err(binding_wait_timeout()),
    }
}

async fn wait_for_candidate(
    wait: &mut SessionQueueWait,
    deadline: Instant,
    retry_at: Option<Instant>,
) -> Result<(), PublicError> {
    let Some(retry_at) = retry_at else {
        return wait_for_epoch(wait, deadline).await;
    };
    tokio::select! {
        changed = wait.changes.changed() => {
            changed.map_err(|_| super::internal_error())
        }
        () = sleep_until(retry_at.min(deadline)) => Ok(()),
    }
}

fn selection_coordination_timeout() -> PublicError {
    rate_limit_error("session candidate selection timed out")
}

fn binding_wait_timeout() -> PublicError {
    rate_limit_error("session binding creation timed out")
}

#[cfg(test)]
mod tests;
