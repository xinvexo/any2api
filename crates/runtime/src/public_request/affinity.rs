use std::{collections::BTreeMap, time::Instant};

use any2api_domain::{
    ModelRouteId, ProtocolDialect, ProtocolOperation, PublicError, PublicErrorCode,
};
use any2api_protocol::api::{IngressAffinity, ProtocolContinuationState};
use tokio::time::{Instant as TokioInstant, timeout_at};

use super::{
    SelectedCandidate,
    response::{internal_error, public_error},
    selection::{FixedSelectionError, select_candidate, select_fixed_candidate},
};
use crate::{
    affinity::{AffinityError, AffinityTarget, BindingLease, ContinuationLookup},
    configuration::PublishedSnapshot,
    routing::{CandidateExclusions, RouteCandidate},
};

mod session;

use session::select_session;

pub(super) struct AffinitySelection {
    pub(super) selected: SelectedCandidate,
    pub(super) target: AffinityTarget,
    pub(super) binding_lease: Option<BindingLease>,
    pub(super) bound: bool,
    pub(super) continuation_state: Option<ProtocolContinuationState>,
}

pub(super) struct AffinitySelectionInput<'a> {
    pub(super) snapshot: &'a PublishedSnapshot,
    pub(super) operation: ProtocolOperation,
    pub(super) affinity: &'a IngressAffinity,
    pub(super) route_id: ModelRouteId,
    pub(super) dialect: ProtocolDialect,
    pub(super) fallback_on_rate_limit: bool,
    pub(super) tiers: &'a BTreeMap<u16, Vec<RouteCandidate>>,
    pub(super) exclusions: &'a CandidateExclusions,
}

pub(super) async fn select(
    input: AffinitySelectionInput<'_>,
) -> Result<AffinitySelection, PublicError> {
    match input.affinity {
        IngressAffinity::None => select_unbound(&input, None).await,
        IngressAffinity::Continuation(raw) => select_continuation(&input, raw).await,
        IngressAffinity::Session(raw) if input.snapshot.affinity_policy().enabled() => {
            select_session(&input, raw).await
        }
        IngressAffinity::Session(_) => select_unbound(&input, None).await,
    }
}

async fn select_continuation(
    input: &AffinitySelectionInput<'_>,
    raw: &str,
) -> Result<AffinitySelection, PublicError> {
    let continuation = acquire_continuation_binding(input, raw).await?;
    let (target, state) = continuation.into_parts();
    select_bound(input, target, state).await
}

async fn acquire_continuation_binding(
    input: &AffinitySelectionInput<'_>,
    raw: &str,
) -> Result<crate::affinity::ResolvedContinuation, PublicError> {
    let snapshot = input.snapshot;
    let policy = snapshot.affinity_policy();
    let wait_deadline = TokioInstant::now() + policy.wait_timeout();
    let mut queue_ticket = None;
    let mut scheduler_changes: Option<tokio::sync::watch::Receiver<u64>> = None;
    loop {
        if queue_ticket.is_some() && TokioInstant::now() >= wait_deadline {
            return Err(binding_lost());
        }
        if let Some(changes) = scheduler_changes.as_mut() {
            let _observed_epoch = *changes.borrow_and_update();
        }
        match snapshot
            .affinity_registry()
            .resolve_continuation(raw, policy.ttl(), |target| {
                find_candidate(target, input.route_id, input.dialect, input.tiers).is_some()
            }) {
            ContinuationLookup::Missing => return Err(binding_lost()),
            ContinuationLookup::Ready(continuation) => return Ok(continuation),
            ContinuationLookup::Pending if queue_ticket.is_none() => {
                let ticket = snapshot
                    .queue_coordinator()
                    .try_ticket(snapshot.queue_policy().max_waiting_requests())
                    .ok_or_else(|| {
                        public_error(PublicErrorCode::LocalRateLimit, "request queue is full")
                    })?;
                scheduler_changes = Some(ticket.subscribe());
                queue_ticket = Some(ticket);
            }
            ContinuationLookup::Pending => {
                let changes = scheduler_changes
                    .as_mut()
                    .expect("queue ticket always carries an epoch subscription");
                match timeout_at(wait_deadline, changes.changed()).await {
                    Ok(Ok(())) => {}
                    Ok(Err(_)) => return Err(internal_error()),
                    Err(_) => return Err(binding_lost()),
                }
            }
        }
    }
}

async fn select_bound(
    input: &AffinitySelectionInput<'_>,
    target: AffinityTarget,
    continuation_state: Option<ProtocolContinuationState>,
) -> Result<AffinitySelection, PublicError> {
    let candidate = find_candidate(&target, input.route_id, input.dialect, input.tiers)
        .ok_or_else(binding_lost)?;
    let selected = select_fixed_candidate(
        input.snapshot,
        candidate,
        input.snapshot.affinity_policy().wait_timeout(),
    )
    .await
    .map_err(FixedSelectionError::into_public_error)?;
    Ok(AffinitySelection {
        selected,
        target,
        binding_lease: None,
        bound: true,
        continuation_state,
    })
}

async fn select_unbound(
    input: &AffinitySelectionInput<'_>,
    binding_lease: Option<BindingLease>,
) -> Result<AffinitySelection, PublicError> {
    let selected = select_candidate(
        input.snapshot,
        input.operation,
        input.route_id,
        input.fallback_on_rate_limit,
        input.tiers,
        input.exclusions,
    )
    .await?;
    Ok(finish_unbound(input, selected, binding_lease))
}

fn finish_unbound(
    input: &AffinitySelectionInput<'_>,
    selected: SelectedCandidate,
    binding_lease: Option<BindingLease>,
) -> AffinitySelection {
    let target = AffinityTarget::from_candidate(input.route_id, input.dialect, &selected.candidate);
    AffinitySelection {
        selected,
        target,
        binding_lease,
        bound: false,
        continuation_state: None,
    }
}

fn find_candidate<'a>(
    target: &AffinityTarget,
    route_id: ModelRouteId,
    dialect: ProtocolDialect,
    tiers: &'a BTreeMap<u16, Vec<RouteCandidate>>,
) -> Option<&'a RouteCandidate> {
    tiers
        .values()
        .flatten()
        .find(|candidate| target.matches_candidate(route_id, dialect, candidate))
}

fn binding_lost() -> PublicError {
    public_error(
        PublicErrorCode::SessionBindingLost,
        "session binding is missing or unavailable",
    )
}

pub(super) fn affinity_error(error: AffinityError) -> PublicError {
    match error {
        AffinityError::Capacity => public_error(
            PublicErrorCode::LocalRateLimit,
            "session affinity capacity is full",
        ),
        AffinityError::IdentityConflict
        | AffinityError::LeaseLost
        | AffinityError::DeadlineExceeded => internal_error(),
        AffinityError::ContinuationCapacity => public_error(
            PublicErrorCode::LocalRateLimit,
            "protocol continuation memory capacity is full",
        ),
        AffinityError::ContinuationTooLarge => public_error(
            PublicErrorCode::UpstreamError,
            "protocol continuation state exceeded its size limit",
        ),
    }
}

pub(super) fn commit_binding(
    lease: Option<BindingLease>,
    target: AffinityTarget,
) -> Result<(), PublicError> {
    match lease {
        Some(lease) => lease.commit(target).map_err(affinity_error),
        None => Ok(()),
    }
}

pub(super) fn commit_binding_before(
    lease: Option<BindingLease>,
    target: AffinityTarget,
    deadline: Instant,
) -> Result<(), PublicError> {
    match lease {
        Some(lease) => lease
            .commit_before(target, deadline)
            .map_err(|error| match error {
                AffinityError::DeadlineExceeded => public_error(
                    PublicErrorCode::UpstreamError,
                    "stream precommit deadline elapsed",
                ),
                other => affinity_error(other),
            }),
        None => Ok(()),
    }
}
