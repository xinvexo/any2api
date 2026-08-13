use tokio::time::Instant;

use super::filter_recorder::RequestFilterRecorder;
use crate::{
    credential::{CredentialFilterKind, RateLimited},
    health::{HealthAcquireError, ReliabilityPolicy, TemporaryUnavailability},
    public_request::SelectedCandidate,
    routing::{CandidateExclusions, CandidateHealthError, RouteCandidate},
};

pub(super) enum TierScan {
    Acquired {
        selected: Box<SelectedCandidate>,
        skipped: u64,
    },
    RateLimited {
        retry_at: Option<Instant>,
    },
    Exhausted {
        outage_retry_at: Option<Instant>,
        cooldown_retry_at: Option<Instant>,
    },
}

pub(super) fn scan(
    policy: ReliabilityPolicy,
    candidates: &[RouteCandidate],
    exclusions: &CandidateExclusions,
    filters: &mut RequestFilterRecorder,
    tie_breaker: u64,
) -> TierScan {
    if candidates.is_empty() {
        return TierScan::Exhausted {
            outage_retry_at: None,
            cooldown_retry_at: None,
        };
    }
    let start = usize::try_from(tie_breaker % candidates.len() as u64)
        .expect("tie breaker is bounded by candidate count");
    let mut outage_retry_at = None;
    let mut cooldown_retry_at = None;
    let mut rate_retry_at = None;
    let mut saw_rate_limit = false;

    for preference in 0..CandidateExclusions::RETRY_PREFERENCE_LEVELS {
        for offset in 0..candidates.len() {
            let candidate = &candidates[(start + offset) % candidates.len()];
            if exclusions.retry_preference(candidate) != preference || !exclusions.allows(candidate)
            {
                continue;
            }
            if let Err(error) = candidate.health_availability(&policy) {
                note_health_error(
                    candidate,
                    error,
                    filters,
                    &mut outage_retry_at,
                    &mut cooldown_retry_at,
                );
                continue;
            }
            let permit = match candidate.binding.try_reserve() {
                Ok(permit) => permit,
                Err(RateLimited { retry_at }) => {
                    filters.record(candidate, CredentialFilterKind::RateLimit);
                    saw_rate_limit = true;
                    rate_retry_at = earliest_optional(rate_retry_at, retry_at);
                    continue;
                }
            };
            let (permit, health) =
                match candidate.acquire_health_with_rpm_reservation(policy, permit) {
                    Ok(acquired) => acquired,
                    Err(error) => {
                        note_health_error(
                            candidate,
                            error,
                            filters,
                            &mut outage_retry_at,
                            &mut cooldown_retry_at,
                        );
                        continue;
                    }
                };
            candidate.record_selection();
            return TierScan::Acquired {
                selected: Box::new(SelectedCandidate {
                    candidate: candidate.clone(),
                    permit,
                    health,
                }),
                skipped: u64::try_from(offset).expect("candidate offset fits u64"),
            };
        }
    }
    if saw_rate_limit {
        TierScan::RateLimited {
            retry_at: rate_retry_at,
        }
    } else {
        TierScan::Exhausted {
            outage_retry_at,
            cooldown_retry_at,
        }
    }
}

fn note_health_error(
    candidate: &RouteCandidate,
    error: CandidateHealthError,
    filters: &mut RequestFilterRecorder,
    outage_retry_at: &mut Option<Instant>,
    cooldown_retry_at: &mut Option<Instant>,
) {
    filters.record(candidate, error.kind());
    if let HealthAcquireError::Temporary(unavailability) = error.source() {
        note_temporary(outage_retry_at, cooldown_retry_at, unavailability);
    }
}

fn note_temporary(
    outage_retry_at: &mut Option<Instant>,
    cooldown_retry_at: &mut Option<Instant>,
    unavailability: TemporaryUnavailability,
) {
    let slot = match unavailability.cause() {
        crate::health::TemporaryUnavailabilityCause::Outage => outage_retry_at,
        crate::health::TemporaryUnavailabilityCause::RateLimitCooldown => cooldown_retry_at,
    };
    *slot = Some(slot.map_or(unavailability.until(), |current| {
        current.min(unavailability.until())
    }));
}

fn earliest_optional(current: Option<Instant>, candidate: Option<Instant>) -> Option<Instant> {
    match candidate {
        Some(candidate) => Some(current.map_or(candidate, |current| current.min(candidate))),
        None => current,
    }
}
