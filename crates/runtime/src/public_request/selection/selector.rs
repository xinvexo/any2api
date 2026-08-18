use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use any2api_domain::{ModelRouteId, PublicError, PublicErrorCode, RoutingCredentialId};
use tokio::time::Instant;

use super::super::{
    SelectedCandidate,
    response::{internal_error, public_error},
};
use super::filter_recorder::RequestFilterRecorder;
use super::{fixed, generation};
use crate::{
    configuration::PublishedSnapshot,
    routing::{CandidateSelectionState, QueueCoordinator, QueueTicket, RouteCandidate},
};

#[cfg(test)]
use crate::routing::QueuePolicy;

pub(in crate::public_request) enum GenerationSelection {
    Acquired(Box<SelectedCandidate>),
    RateLimited(Option<Instant>),
    TemporarilyUnavailable(Instant),
    RetryDeferred(Instant),
    NoCandidates,
}

/// Queue capacity and absolute wait limits shared by every re-plan before an
/// Attempt starts. A configuration wake-up must not forfeit the ticket or
/// restart either timeout.
#[derive(Default)]
pub(in crate::public_request) struct SelectionWaitState {
    inner: Mutex<SelectionWaitStateInner>,
}

#[derive(Default)]
struct SelectionWaitStateInner {
    queue_deadline: Option<Instant>,
    binding_deadline: Option<Instant>,
    queue_ticket: Option<QueueTicket>,
}

impl SelectionWaitState {
    pub(in crate::public_request) fn queue(&self, timeout: Duration) -> Instant {
        let mut inner = self.inner.lock().expect("selection wait lock poisoned");
        *inner
            .queue_deadline
            .get_or_insert_with(|| Instant::now() + timeout)
    }

    pub(in crate::public_request) fn binding(&self, timeout: Duration) -> Instant {
        let mut inner = self.inner.lock().expect("selection wait lock poisoned");
        *inner
            .binding_deadline
            .get_or_insert_with(|| Instant::now() + timeout)
    }

    pub(in crate::public_request) fn queue_changes(
        &self,
        coordinator: &Arc<QueueCoordinator>,
        max_waiting_requests: u32,
    ) -> Option<tokio::sync::watch::Receiver<u64>> {
        let mut inner = self.inner.lock().expect("selection wait lock poisoned");
        if inner.queue_ticket.is_none() {
            inner.queue_ticket = coordinator.try_ticket(max_waiting_requests);
        }
        inner.queue_ticket.as_ref().map(QueueTicket::subscribe)
    }
}

pub(in crate::public_request) struct CandidateSelector<'a> {
    policy_snapshot: &'a PublishedSnapshot,
    routing_snapshot: &'a PublishedSnapshot,
    route_id: ModelRouteId,
    fallback_on_rate_limit: bool,
    tiers: &'a BTreeMap<u16, Vec<RouteCandidate>>,
    selection_state: &'a CandidateSelectionState,
    credential_eligible: &'a (dyn Fn(RoutingCredentialId) -> bool + Sync),
    filters: RequestFilterRecorder,
}

impl<'a> CandidateSelector<'a> {
    pub(in crate::public_request) fn new(
        policy_snapshot: &'a PublishedSnapshot,
        routing_snapshot: &'a PublishedSnapshot,
        route_id: ModelRouteId,
        fallback_on_rate_limit: bool,
        tiers: &'a BTreeMap<u16, Vec<RouteCandidate>>,
        selection_state: &'a CandidateSelectionState,
        credential_eligible: &'a (dyn Fn(RoutingCredentialId) -> bool + Sync),
    ) -> Self {
        Self {
            policy_snapshot,
            routing_snapshot,
            route_id,
            fallback_on_rate_limit,
            tiers,
            selection_state,
            credential_eligible,
            filters: RequestFilterRecorder::default(),
        }
    }

    pub(in crate::public_request) fn try_select(
        &mut self,
    ) -> Result<GenerationSelection, PublicError> {
        generation::try_select(generation::GenerationSelectionInput {
            policy_snapshot: self.policy_snapshot,
            routing_snapshot: self.routing_snapshot,
            route_id: self.route_id,
            fallback_on_rate_limit: self.fallback_on_rate_limit,
            tiers: self.tiers,
            selection_state: self.selection_state,
            credential_eligible: self.credential_eligible,
            filters: &mut self.filters,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::public_request) enum FixedSelectionError {
    QueueFull,
    Timeout,
    Unavailable,
    Internal,
}

impl FixedSelectionError {
    pub(in crate::public_request) fn into_public_error(self) -> PublicError {
        match self {
            Self::QueueFull => rate_limit_error("request queue is full"),
            Self::Timeout => rate_limit_error("bound credential has exhausted its local RPM"),
            Self::Unavailable => public_error(
                PublicErrorCode::SessionBindingLost,
                "session binding is unavailable",
            ),
            Self::Internal => internal_error(),
        }
    }
}

pub(in crate::public_request) async fn select_candidate(
    input: CandidateSelectionInput<'_>,
) -> Result<SelectedCandidate, PublicError> {
    let CandidateSelectionInput {
        policy_snapshot,
        routing_snapshot,
        route_id,
        fallback_on_rate_limit,
        tiers,
        selection_state,
        credential_eligible,
        wait_state,
        ..
    } = input;
    let mut selector = CandidateSelector::new(
        policy_snapshot,
        routing_snapshot,
        route_id,
        fallback_on_rate_limit,
        tiers,
        selection_state,
        credential_eligible,
    );
    generation::select_with_queue(
        policy_snapshot.queue_coordinator(),
        policy_snapshot.queue_policy(),
        wait_state,
        || selector.try_select(),
    )
    .await
}

pub(in crate::public_request) struct CandidateSelectionInput<'a> {
    pub(in crate::public_request) policy_snapshot: &'a PublishedSnapshot,
    pub(in crate::public_request) routing_snapshot: &'a PublishedSnapshot,
    pub(in crate::public_request) route_id: ModelRouteId,
    pub(in crate::public_request) fallback_on_rate_limit: bool,
    pub(in crate::public_request) tiers: &'a BTreeMap<u16, Vec<RouteCandidate>>,
    pub(in crate::public_request) selection_state: &'a CandidateSelectionState,
    pub(in crate::public_request) credential_eligible:
        &'a (dyn Fn(RoutingCredentialId) -> bool + Sync),
    pub(in crate::public_request) wait_state: &'a SelectionWaitState,
}

pub(in crate::public_request) async fn select_fixed_candidate(
    policy_snapshot: &PublishedSnapshot,
    candidate: &RouteCandidate,
    wait_state: &SelectionWaitState,
) -> Result<SelectedCandidate, FixedSelectionError> {
    fixed::select(
        policy_snapshot,
        candidate,
        policy_snapshot.affinity_policy().wait_timeout(),
        wait_state,
    )
    .await
}

#[cfg(test)]
pub(super) async fn select_generation_candidate(
    coordinator: &Arc<QueueCoordinator>,
    policy: QueuePolicy,
    try_select: impl FnMut() -> Result<GenerationSelection, PublicError>,
) -> Result<SelectedCandidate, PublicError> {
    let wait_state = SelectionWaitState::default();
    generation::select_with_queue(coordinator, policy, &wait_state, try_select).await
}

#[cfg(test)]
pub(super) async fn wait_for_generation_candidate(
    coordinator: &Arc<QueueCoordinator>,
    policy: QueuePolicy,
    try_select: impl FnMut() -> Result<GenerationSelection, PublicError>,
) -> Result<SelectedCandidate, PublicError> {
    let wait_state = SelectionWaitState::default();
    generation::wait_for_candidate(coordinator, policy, &wait_state, try_select).await
}

#[cfg(test)]
pub(super) fn try_select_generation_candidate_for_test(
    fallback_on_rate_limit: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<GenerationSelection, PublicError> {
    generation::try_select_for_test(fallback_on_rate_limit, tiers, tie_breaker)
}

#[cfg(test)]
pub(super) fn try_select_generation_candidate_with_state_for_test(
    fallback_on_rate_limit: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    selection_state: &CandidateSelectionState,
    credential_eligible: &(dyn Fn(RoutingCredentialId) -> bool + Sync),
    tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<GenerationSelection, PublicError> {
    generation::try_select_for_test_with_state(
        fallback_on_rate_limit,
        tiers,
        selection_state,
        credential_eligible,
        tie_breaker,
    )
}

#[cfg(test)]
pub(super) fn try_select_fixed_candidate_for_test(
    policy: crate::health::ReliabilityPolicy,
    candidate: &RouteCandidate,
    after_reservation: impl FnOnce(),
) -> Result<Option<SelectedCandidate>, FixedSelectionError> {
    fixed::try_selected_for_test(policy, candidate, after_reservation)
}

pub(in crate::public_request) fn rate_limit_error(message: &'static str) -> PublicError {
    public_error(PublicErrorCode::LocalRateLimit, message)
}

pub(in crate::public_request) fn rate_limited(
    message: &'static str,
    retry_at: Option<Instant>,
) -> PublicError {
    let error = rate_limit_error(message);
    retry_at.map_or(error.clone(), |retry_at| {
        let delay = retry_at.saturating_duration_since(Instant::now());
        let seconds = delay
            .as_secs()
            .saturating_add(u64::from(delay.subsec_nanos() > 0));
        error.with_retry_after_seconds(seconds)
    })
}

pub(in crate::public_request) fn no_available_credentials() -> PublicError {
    public_error(
        PublicErrorCode::NoAvailableCredential,
        "no eligible provider credential is available",
    )
}

pub(in crate::public_request) fn temporarily_unavailable(retry_at: Instant) -> PublicError {
    let delay = retry_at.saturating_duration_since(Instant::now());
    let seconds = delay
        .as_secs()
        .saturating_add(u64::from(delay.subsec_nanos() > 0));
    no_available_credentials().with_retry_after_seconds(seconds)
}
