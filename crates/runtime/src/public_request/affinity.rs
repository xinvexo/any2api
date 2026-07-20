use std::{collections::BTreeMap, time::Instant};

use any2api_domain::{
    AffinityMode, ModelRouteId, ProtocolDialect, ProtocolOperation, PublicError, PublicErrorCode,
};
use any2api_protocol::api::IngressAffinity;
use tokio::time::timeout;

use super::{
    SelectedCandidate,
    response::{internal_error, public_error},
    selection::{FixedSelectionError, select_candidate, select_fixed_candidate},
};
use crate::{
    affinity::{AffinityError, AffinityTarget, SoftBindingLease, SoftBindingStart},
    published_snapshot::PublishedSnapshot,
    route_candidates::{CandidateExclusions, RouteCandidate},
};

pub(super) struct AffinitySelection {
    pub(super) selected: SelectedCandidate,
    pub(super) target: AffinityTarget,
    pub(super) soft_lease: Option<SoftBindingLease>,
    pub(super) fixed: bool,
}

pub(super) struct AffinitySelectionInput<'a> {
    pub(super) snapshot: &'a PublishedSnapshot,
    pub(super) operation: ProtocolOperation,
    pub(super) affinity: &'a IngressAffinity,
    pub(super) route_id: ModelRouteId,
    pub(super) dialect: ProtocolDialect,
    pub(super) fallback_on_saturation: bool,
    pub(super) tiers: &'a BTreeMap<u16, Vec<RouteCandidate>>,
    pub(super) exclusions: &'a CandidateExclusions,
}

pub(super) async fn select(
    input: AffinitySelectionInput<'_>,
) -> Result<AffinitySelection, PublicError> {
    if let IngressAffinity::Hard(raw) = input.affinity {
        return select_hard(&input, raw).await;
    }
    if let IngressAffinity::Soft(raw) = input.affinity
        && input.snapshot.affinity_policy().soft_enabled()
    {
        return select_soft(&input, raw).await;
    }
    select_unbound(&input, None).await
}

async fn select_hard(
    input: &AffinitySelectionInput<'_>,
    raw: &str,
) -> Result<AffinitySelection, PublicError> {
    let target = input
        .snapshot
        .affinity_registry()
        .resolve_hard(raw, input.snapshot.affinity_policy().hard_ttl())
        .ok_or_else(binding_lost)?;
    let candidate = find_candidate(&target, input.route_id, input.dialect, input.tiers)
        .ok_or_else(binding_lost)?;
    let selected = select_fixed_candidate(
        input.snapshot,
        candidate,
        input.snapshot.affinity_policy().fixed_wait_timeout(),
    )
    .await
    .map_err(FixedSelectionError::into_public_error)?;
    Ok(AffinitySelection {
        selected,
        target,
        soft_lease: None,
        fixed: true,
    })
}

async fn select_soft(
    input: &AffinitySelectionInput<'_>,
    raw: &str,
) -> Result<AffinitySelection, PublicError> {
    let policy = input.snapshot.affinity_policy();
    loop {
        let start = input
            .snapshot
            .affinity_registry()
            .begin_soft(
                input.dialect,
                input.route_id,
                raw,
                policy.soft_ttl(),
                policy.fixed_wait_timeout(),
            )
            .map_err(affinity_error)?;
        match start {
            SoftBindingStart::Create(lease) => {
                return select_unbound(input, Some(lease)).await;
            }
            SoftBindingStart::Wait(mut wait) => {
                match timeout(policy.fixed_wait_timeout(), wait.changed()).await {
                    Ok(Ok(())) => continue,
                    Ok(Err(_)) => return Err(internal_error()),
                    Err(_) => {
                        return Err(public_error(
                            PublicErrorCode::LocalConcurrencyLimit,
                            "session binding creation timed out",
                        ));
                    }
                }
            }
            SoftBindingStart::Bound(binding) => {
                let Some(candidate) =
                    find_candidate(binding.target(), input.route_id, input.dialect, input.tiers)
                else {
                    if policy.soft_mode() == AffinityMode::Strict {
                        return Err(binding_lost());
                    }
                    input.snapshot.affinity_registry().invalidate_soft(&binding);
                    continue;
                };
                if !input.exclusions.allows(candidate) {
                    if policy.soft_mode() == AffinityMode::Strict {
                        return Err(binding_lost());
                    }
                    input.snapshot.affinity_registry().invalidate_soft(&binding);
                    continue;
                }
                let wait_timeout = match policy.soft_mode() {
                    AffinityMode::Prefer => policy.soft_prefer_wait_timeout(),
                    AffinityMode::Strict => policy.fixed_wait_timeout(),
                };
                match select_fixed_candidate(input.snapshot, candidate, wait_timeout).await {
                    Ok(selected) => {
                        return Ok(AffinitySelection {
                            selected,
                            target: binding.target().clone(),
                            soft_lease: None,
                            fixed: policy.soft_mode() == AffinityMode::Strict,
                        });
                    }
                    Err(FixedSelectionError::Timeout | FixedSelectionError::Unavailable)
                        if policy.soft_mode() == AffinityMode::Prefer =>
                    {
                        input.snapshot.affinity_registry().invalidate_soft(&binding);
                    }
                    Err(error) => return Err(error.into_public_error()),
                }
            }
        }
    }
}

async fn select_unbound(
    input: &AffinitySelectionInput<'_>,
    soft_lease: Option<SoftBindingLease>,
) -> Result<AffinitySelection, PublicError> {
    let selected = select_candidate(
        input.snapshot,
        input.operation,
        input.route_id,
        input.fallback_on_saturation,
        input.tiers,
        input.exclusions,
    )
    .await?;
    let target = AffinityTarget::from_candidate(input.route_id, input.dialect, &selected.candidate);
    Ok(AffinitySelection {
        selected,
        target,
        soft_lease,
        fixed: false,
    })
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

fn affinity_error(error: AffinityError) -> PublicError {
    match error {
        AffinityError::Capacity => public_error(
            PublicErrorCode::LocalConcurrencyLimit,
            "session affinity capacity is full",
        ),
        AffinityError::IdentityConflict
        | AffinityError::LeaseLost
        | AffinityError::DeadlineExceeded => internal_error(),
    }
}

pub(super) fn commit_soft_binding(
    lease: Option<SoftBindingLease>,
    target: AffinityTarget,
) -> Result<(), PublicError> {
    match lease {
        Some(lease) => lease.commit(target).map_err(affinity_error),
        None => Ok(()),
    }
}

pub(super) fn commit_soft_binding_before(
    lease: Option<SoftBindingLease>,
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
