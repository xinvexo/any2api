use std::{collections::BTreeMap, time::Duration};

#[cfg(test)]
use std::sync::Arc;

use any2api_domain::{ModelRouteId, ProtocolOperation, PublicError, PublicErrorCode};
use tokio::time::Instant;

use super::super::{
    SelectedCandidate,
    response::{internal_error, public_error},
};
use super::{fixed, generation};
#[cfg(test)]
use crate::routing::{QueueCoordinator, QueuePolicy};
use crate::{
    configuration::PublishedSnapshot,
    routing::{CandidateExclusions, RouteCandidate},
};

pub(super) enum GenerationSelection {
    Acquired(Box<SelectedCandidate>),
    RateLimited(Option<Instant>),
    TemporarilyUnavailable(Instant),
    NoCandidates,
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
    snapshot: &PublishedSnapshot,
    _operation: ProtocolOperation,
    route_id: ModelRouteId,
    fallback_on_rate_limit: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    exclusions: &CandidateExclusions,
) -> Result<SelectedCandidate, PublicError> {
    let try_select = || {
        generation::try_select(
            snapshot,
            route_id,
            fallback_on_rate_limit,
            tiers,
            exclusions,
        )
    };
    generation::select_with_queue(
        snapshot.queue_coordinator(),
        snapshot.queue_policy(),
        try_select,
    )
    .await
}

pub(in crate::public_request) async fn select_fixed_candidate(
    snapshot: &PublishedSnapshot,
    candidate: &RouteCandidate,
    wait_timeout: Duration,
) -> Result<SelectedCandidate, FixedSelectionError> {
    fixed::select(snapshot, candidate, wait_timeout).await
}

#[cfg(test)]
pub(super) async fn select_generation_candidate(
    coordinator: &Arc<QueueCoordinator>,
    policy: QueuePolicy,
    try_select: impl FnMut() -> Result<GenerationSelection, PublicError>,
) -> Result<SelectedCandidate, PublicError> {
    generation::select_with_queue(coordinator, policy, try_select).await
}

#[cfg(test)]
pub(super) async fn wait_for_generation_candidate(
    coordinator: &Arc<QueueCoordinator>,
    policy: QueuePolicy,
    try_select: impl FnMut() -> Result<GenerationSelection, PublicError>,
) -> Result<SelectedCandidate, PublicError> {
    generation::wait_for_candidate(coordinator, policy, try_select).await
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
pub(super) fn try_select_fixed_candidate_for_test(
    policy: crate::health::ReliabilityPolicy,
    candidate: &RouteCandidate,
) -> Result<Option<SelectedCandidate>, FixedSelectionError> {
    fixed::try_selected_for_test(policy, candidate)
}

pub(super) fn rate_limit_error(message: &'static str) -> PublicError {
    public_error(PublicErrorCode::LocalRateLimit, message)
}

pub(super) fn rate_limited(message: &'static str, retry_at: Option<Instant>) -> PublicError {
    let error = rate_limit_error(message);
    retry_at.map_or(error.clone(), |retry_at| {
        let delay = retry_at.saturating_duration_since(Instant::now());
        let seconds = delay
            .as_secs()
            .saturating_add(u64::from(delay.subsec_nanos() > 0));
        error.with_retry_after_seconds(seconds)
    })
}

pub(super) fn no_available_credentials() -> PublicError {
    public_error(
        PublicErrorCode::NoAvailableCredential,
        "no eligible provider credential is available",
    )
}

pub(super) fn temporarily_unavailable(retry_at: Instant) -> PublicError {
    let delay = retry_at.saturating_duration_since(Instant::now());
    let seconds = delay
        .as_secs()
        .saturating_add(u64::from(delay.subsec_nanos() > 0));
    no_available_credentials().with_retry_after_seconds(seconds)
}
