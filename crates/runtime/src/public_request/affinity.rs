use std::collections::BTreeMap;

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
    route_candidates::RouteCandidate,
};

pub(super) struct AffinitySelection {
    pub(super) selected: SelectedCandidate,
    pub(super) target: AffinityTarget,
    pub(super) soft_lease: Option<SoftBindingLease>,
}

pub(super) async fn select(
    snapshot: &PublishedSnapshot,
    operation: ProtocolOperation,
    affinity: &IngressAffinity,
    route_id: ModelRouteId,
    dialect: ProtocolDialect,
    fallback_on_saturation: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
) -> Result<AffinitySelection, PublicError> {
    if let IngressAffinity::Hard(raw) = affinity {
        return select_hard(snapshot, raw, route_id, dialect, tiers).await;
    }
    if let IngressAffinity::Soft(raw) = affinity
        && snapshot.affinity_policy().soft_enabled()
    {
        return select_soft(
            snapshot,
            operation,
            raw,
            route_id,
            dialect,
            fallback_on_saturation,
            tiers,
        )
        .await;
    }
    select_unbound(
        snapshot,
        operation,
        route_id,
        dialect,
        fallback_on_saturation,
        tiers,
        None,
    )
    .await
}

async fn select_hard(
    snapshot: &PublishedSnapshot,
    raw: &str,
    route_id: ModelRouteId,
    dialect: ProtocolDialect,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
) -> Result<AffinitySelection, PublicError> {
    let target = snapshot
        .affinity_registry()
        .resolve_hard(raw, snapshot.affinity_policy().hard_ttl())
        .ok_or_else(binding_lost)?;
    let candidate = find_candidate(&target, route_id, dialect, tiers).ok_or_else(binding_lost)?;
    let selected = select_fixed_candidate(
        snapshot,
        candidate,
        snapshot.affinity_policy().fixed_wait_timeout(),
    )
    .await
    .map_err(FixedSelectionError::into_public_error)?;
    Ok(AffinitySelection {
        selected,
        target,
        soft_lease: None,
    })
}

async fn select_soft(
    snapshot: &PublishedSnapshot,
    operation: ProtocolOperation,
    raw: &str,
    route_id: ModelRouteId,
    dialect: ProtocolDialect,
    fallback_on_saturation: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
) -> Result<AffinitySelection, PublicError> {
    let policy = snapshot.affinity_policy();
    loop {
        let start = snapshot
            .affinity_registry()
            .begin_soft(
                dialect,
                route_id,
                raw,
                policy.soft_ttl(),
                policy.fixed_wait_timeout(),
            )
            .map_err(affinity_error)?;
        match start {
            SoftBindingStart::Create(lease) => {
                return select_unbound(
                    snapshot,
                    operation,
                    route_id,
                    dialect,
                    fallback_on_saturation,
                    tiers,
                    Some(lease),
                )
                .await;
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
                let Some(candidate) = find_candidate(binding.target(), route_id, dialect, tiers)
                else {
                    if policy.soft_mode() == AffinityMode::Strict {
                        return Err(binding_lost());
                    }
                    snapshot.affinity_registry().invalidate_soft(&binding);
                    continue;
                };
                let wait_timeout = match policy.soft_mode() {
                    AffinityMode::Prefer => policy.soft_prefer_wait_timeout(),
                    AffinityMode::Strict => policy.fixed_wait_timeout(),
                };
                match select_fixed_candidate(snapshot, candidate, wait_timeout).await {
                    Ok(selected) => {
                        return Ok(AffinitySelection {
                            selected,
                            target: binding.target().clone(),
                            soft_lease: None,
                        });
                    }
                    Err(FixedSelectionError::Timeout)
                        if policy.soft_mode() == AffinityMode::Prefer =>
                    {
                        snapshot.affinity_registry().invalidate_soft(&binding);
                    }
                    Err(error) => return Err(error.into_public_error()),
                }
            }
        }
    }
}

async fn select_unbound(
    snapshot: &PublishedSnapshot,
    operation: ProtocolOperation,
    route_id: ModelRouteId,
    dialect: ProtocolDialect,
    fallback_on_saturation: bool,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    soft_lease: Option<SoftBindingLease>,
) -> Result<AffinitySelection, PublicError> {
    let selected =
        select_candidate(snapshot, operation, route_id, fallback_on_saturation, tiers).await?;
    let target = AffinityTarget::from_candidate(route_id, dialect, &selected.candidate);
    Ok(AffinitySelection {
        selected,
        target,
        soft_lease,
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
        AffinityError::IdentityConflict | AffinityError::LeaseLost => internal_error(),
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
