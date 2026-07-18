use std::cmp::Ordering;

use crate::credential_runtime::{ConcurrencyPermit, CredentialCapacity, CredentialRuntimeBinding};

#[derive(Debug)]
pub enum SelectAndAcquireResult {
    Acquired(ConcurrencyPermit),
    AtCapacity,
    NoCandidates,
}

#[must_use]
pub fn select_and_try_acquire(
    candidates: &[CredentialRuntimeBinding],
    tie_breaker: u64,
) -> SelectAndAcquireResult {
    if candidates.is_empty() {
        return SelectAndAcquireResult::NoCandidates;
    }

    loop {
        let Some(index) = select_available(candidates, tie_breaker) else {
            return SelectAndAcquireResult::AtCapacity;
        };
        if let Some(permit) = candidates[index].try_acquire() {
            return SelectAndAcquireResult::Acquired(permit);
        }
    }
}

fn select_available(candidates: &[CredentialRuntimeBinding], tie_breaker: u64) -> Option<usize> {
    let start = usize::try_from(tie_breaker % candidates.len() as u64)
        .expect("tie breaker is bounded by candidate count");
    let mut best: Option<(usize, CredentialCapacity)> = None;

    for (index, candidate) in candidates.iter().enumerate() {
        let capacity = candidate.capacity();
        if capacity.is_full() {
            continue;
        }
        let replace = best.is_none_or(|(best_index, best_capacity)| {
            compare_load(capacity, best_capacity).then_with(|| {
                cyclic_rank(index, start, candidates.len()).cmp(&cyclic_rank(
                    best_index,
                    start,
                    candidates.len(),
                ))
            }) == Ordering::Less
        });
        if replace {
            best = Some((index, capacity));
        }
    }

    best.map(|(index, _)| index)
}

fn compare_load(left: CredentialCapacity, right: CredentialCapacity) -> Ordering {
    let left_scaled = u64::from(left.in_flight()) * u64::from(right.max_concurrency());
    let right_scaled = u64::from(right.in_flight()) * u64::from(left.max_concurrency());
    left_scaled.cmp(&right_scaled)
}

const fn cyclic_rank(index: usize, start: usize, length: usize) -> usize {
    (index + length - start) % length
}
