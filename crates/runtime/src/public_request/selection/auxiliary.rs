use std::{collections::BTreeMap, sync::Arc};

use any2api_domain::{FallbackTier, ModelRouteId, PublicError};

use super::super::SelectedCandidate;
use crate::{
    auxiliary_scheduler::{AuxiliaryScheduler, AuxiliarySelectAndAcquireResult},
    health::{HealthAcquireError, ReliabilityPolicy},
    published_snapshot::PublishedSnapshot,
    route_candidates::{CandidateExclusions, RouteCandidate},
};

pub(super) fn select(
    snapshot: &PublishedSnapshot,
    route_id: ModelRouteId,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    exclusions: &CandidateExclusions,
) -> Result<SelectedCandidate, PublicError> {
    select_with(
        snapshot.reliability_policy(),
        snapshot.auxiliary_scheduler(),
        tiers,
        exclusions,
        |tier| {
            snapshot
                .route_tier_cursor(route_id, FallbackTier::new(tier))
                .map(|cursor| cursor.reserve())
        },
    )
}

fn select_with(
    policy: ReliabilityPolicy,
    scheduler: &Arc<AuxiliaryScheduler>,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    exclusions: &CandidateExclusions,
    mut tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<SelectedCandidate, PublicError> {
    for (tier, candidates) in tiers {
        let eligible = candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                exclusions.allows(candidate) && candidate.health_availability(&policy).is_ok()
            })
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            continue;
        }
        let mut eligible = eligible;
        while !eligible.is_empty() {
            let bindings = eligible
                .iter()
                .map(|(_, candidate)| candidate.binding.clone())
                .collect::<Vec<_>>();
            let tie_breaker =
                tie_breaker(*tier).ok_or_else(crate::public_request::response::internal_error)?;
            match scheduler.select_index_and_try_acquire(&bindings, tie_breaker) {
                AuxiliarySelectAndAcquireResult::Acquired { index, permit } => {
                    let candidate = eligible[index].1;
                    let health = match candidate.acquire_health(policy) {
                        Ok(health) => health,
                        Err(HealthAcquireError::Temporary(_) | HealthAcquireError::Permanent) => {
                            drop(permit);
                            eligible.swap_remove(index);
                            continue;
                        }
                    };
                    return Ok(SelectedCandidate {
                        candidate: candidate.clone(),
                        permit: super::super::RequestPermit::Auxiliary(permit),
                        health,
                    });
                }
                AuxiliarySelectAndAcquireResult::AtCapacity => {
                    return Err(crate::public_request::selection::capacity_error(
                        "auxiliary request capacity is full",
                    ));
                }
                AuxiliarySelectAndAcquireResult::NoCandidates => break,
            }
        }
    }
    Err(crate::public_request::selection::no_available_credentials())
}

#[cfg(test)]
pub(super) fn select_for_test(
    scheduler: &Arc<AuxiliaryScheduler>,
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<SelectedCandidate, PublicError> {
    select_with(
        ReliabilityPolicy::from_settings(
            any2api_domain::SettingsConfiguration::defaults().reliability(),
        ),
        scheduler,
        tiers,
        &CandidateExclusions::default(),
        tie_breaker,
    )
}
