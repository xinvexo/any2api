use tokio::time::Instant;

use crate::credential::{CredentialRuntimeBinding, RateLimited, RoutingPermit};

#[derive(Debug)]
pub enum SelectAndReserveResult {
    Reserved(RoutingPermit),
    RateLimited { retry_at: Option<Instant> },
    NoCandidates,
}

#[must_use]
pub fn select_and_try_reserve(
    candidates: &[CredentialRuntimeBinding],
    tie_breaker: u64,
) -> SelectAndReserveResult {
    match select_index_and_try_reserve(candidates, tie_breaker) {
        IndexedSelectAndReserveResult::Reserved { permit, .. } => {
            SelectAndReserveResult::Reserved(permit)
        }
        IndexedSelectAndReserveResult::RateLimited { retry_at } => {
            SelectAndReserveResult::RateLimited { retry_at }
        }
        IndexedSelectAndReserveResult::NoCandidates => SelectAndReserveResult::NoCandidates,
    }
}

pub(crate) enum IndexedSelectAndReserveResult {
    Reserved { index: usize, permit: RoutingPermit },
    RateLimited { retry_at: Option<Instant> },
    NoCandidates,
}

pub(crate) fn select_index_and_try_reserve(
    candidates: &[CredentialRuntimeBinding],
    tie_breaker: u64,
) -> IndexedSelectAndReserveResult {
    if candidates.is_empty() {
        return IndexedSelectAndReserveResult::NoCandidates;
    }

    let start = usize::try_from(tie_breaker % candidates.len() as u64)
        .expect("tie breaker is bounded by candidate count");
    let mut retry_at = None;
    for offset in 0..candidates.len() {
        let index = (start + offset) % candidates.len();
        match candidates[index].try_reserve() {
            Ok(permit) => return IndexedSelectAndReserveResult::Reserved { index, permit },
            Err(RateLimited {
                retry_at: candidate,
            }) => {
                if let Some(candidate) = candidate {
                    retry_at =
                        Some(retry_at.map_or(candidate, |current: Instant| current.min(candidate)));
                }
            }
        }
    }
    IndexedSelectAndReserveResult::RateLimited { retry_at }
}
