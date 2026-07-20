use std::{collections::HashMap, sync::Arc, time::Duration};

use any2api_domain::{CredentialId, PublicError, PublicErrorCode, UpstreamErrorKind};
use any2api_protocol::api::ProtocolAdapter;
use any2api_provider::api::ProviderRegistry;
use any2api_transport::api::{TransportFailureScope, TransportManager};
use tokio::time::{Instant, timeout};

use super::{
    PublicResponse, affinity,
    planning::PlannedRequest,
    response::public_error,
    upstream::{self, AttemptFailure},
};
use crate::{
    health::ReliabilityPolicy, published_snapshot::PublishedSnapshot,
    request_telemetry::RequestRecorder, route_candidates::CandidateExclusions,
};

pub(super) async fn execute(
    snapshot: Arc<PublishedSnapshot>,
    adapter: Arc<dyn ProtocolAdapter>,
    plan: PlannedRequest,
    providers: &ProviderRegistry,
    transport: &dyn TransportManager,
    recorder: RequestRecorder,
) -> Result<PublicResponse, PublicError> {
    let policy = snapshot.reliability_policy();
    let mut budget = RetryBudget::new(policy);
    let mut exclusions = CandidateExclusions::default();
    let mut previous_error = None;
    let services = upstream::UpstreamServices {
        snapshot: snapshot.as_ref(),
        providers,
        transport,
    };

    loop {
        let remaining = budget.remaining();
        if remaining.is_zero() {
            return Err(previous_error.unwrap_or_else(budget_exhausted));
        }
        let selection = timeout(
            remaining,
            affinity::select(affinity::AffinitySelectionInput {
                snapshot: snapshot.as_ref(),
                operation: plan.decoded.operation,
                affinity: &plan.decoded.affinity,
                route_id: plan.route_id,
                dialect: plan.dialect,
                fallback_on_saturation: plan.fallback_on_saturation,
                tiers: &plan.tiers,
                exclusions: &exclusions,
            }),
        )
        .await;
        let affinity = match selection {
            Ok(Ok(selection)) => selection,
            Ok(Err(error)) => return Err(previous_error.unwrap_or(error)),
            Err(_) => return Err(previous_error.unwrap_or_else(budget_exhausted)),
        };
        let credential_id = affinity.selected.candidate.credential_id;
        let Some(attempt_no) = budget.register_attempt(credential_id) else {
            return Err(previous_error.unwrap_or_else(budget_exhausted));
        };
        let attempt_recorder = recorder.begin_attempt(attempt_no, &affinity.selected.candidate);
        let remaining = budget.remaining();
        let attempt = if plan.decoded.stream {
            timeout(
                remaining,
                upstream::execute_stream_attempt(
                    services,
                    Arc::clone(&adapter),
                    plan.decoded.clone(),
                    plan.public_model.clone(),
                    affinity,
                    attempt_recorder,
                ),
            )
            .await
            .map_err(|_| budget_exhausted())?
        } else {
            timeout(
                remaining,
                upstream::execute_buffered_attempt(
                    services,
                    adapter.as_ref(),
                    plan.decoded.clone(),
                    &plan.public_model,
                    affinity,
                    attempt_recorder,
                ),
            )
            .await
            .map_err(|_| budget_exhausted())?
            .map(PublicResponse::from)
        };
        match attempt {
            Ok(response) => return Ok(response),
            Err(failure) => {
                let public = failure.public_error();
                if !should_retry(&failure) || !budget.can_retry() {
                    return Err(public);
                }
                if !failure.fixed() {
                    exclude_failed_path(&mut exclusions, &failure);
                }
                let delay = budget.next_delay();
                if delay >= budget.remaining() {
                    return Err(public);
                }
                previous_error = Some(public);
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}

fn should_retry(failure: &AttemptFailure) -> bool {
    failure.is_retry_candidate() && failure.retry_safety().allows_automatic_retry()
}

fn exclude_failed_path(exclusions: &mut CandidateExclusions, failure: &AttemptFailure) {
    let Some(candidate) = failure.candidate() else {
        return;
    };
    match failure {
        AttemptFailure::Transport { .. } => match failure.transport_failure_scope() {
            Some(TransportFailureScope::Endpoint) => {
                exclusions.exclude_endpoint(candidate.endpoint_id);
            }
            Some(TransportFailureScope::Proxy) => {
                exclusions.exclude_proxy(candidate.proxy_id);
            }
            Some(TransportFailureScope::Unattributed) => {
                exclusions.exclude_credential(candidate.credential_id);
            }
            None => {}
        },
        AttemptFailure::Upstream { classification, .. } => match classification.kind() {
            UpstreamErrorKind::PermissionDenied
            | UpstreamErrorKind::QuotaExhausted
            | UpstreamErrorKind::RateLimited
            | UpstreamErrorKind::ModelUnavailable => {
                exclusions.exclude_credential(candidate.credential_id);
            }
            UpstreamErrorKind::Transient => {
                exclusions.exclude_endpoint(candidate.endpoint_id);
            }
            _ => {}
        },
        AttemptFailure::Public(_) => {}
    }
}

fn budget_exhausted() -> PublicError {
    public_error(
        PublicErrorCode::UpstreamError,
        "upstream precommit retry budget was exhausted",
    )
}

struct RetryBudget {
    policy: ReliabilityPolicy,
    deadline: Instant,
    attempts: u32,
    switches: u32,
    last_credential: Option<CredentialId>,
    attempts_by_credential: HashMap<CredentialId, u32>,
}

impl RetryBudget {
    fn new(policy: ReliabilityPolicy) -> Self {
        Self {
            policy,
            deadline: Instant::now() + policy.precommit_total_budget,
            attempts: 0,
            switches: 0,
            last_credential: None,
            attempts_by_credential: HashMap::new(),
        }
    }

    fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    fn register_attempt(&mut self, credential_id: CredentialId) -> Option<u32> {
        if self.attempts >= self.policy.max_total_attempts {
            return None;
        }
        if self
            .last_credential
            .is_some_and(|previous| previous != credential_id)
        {
            if self.switches >= self.policy.max_credential_switches {
                return None;
            }
            self.switches += 1;
        }
        let prior = self
            .attempts_by_credential
            .get(&credential_id)
            .copied()
            .unwrap_or(0);
        if prior > self.policy.max_same_credential_retries {
            return None;
        }
        self.attempts_by_credential.insert(credential_id, prior + 1);
        self.last_credential = Some(credential_id);
        self.attempts += 1;
        Some(self.attempts)
    }

    fn can_retry(&self) -> bool {
        self.attempts < self.policy.max_total_attempts && !self.remaining().is_zero()
    }

    fn next_delay(&self) -> Duration {
        let exponent = self.attempts.saturating_sub(1).min(31);
        let multiplier = 1_u32 << exponent;
        let base = self
            .policy
            .base_delay
            .saturating_mul(multiplier)
            .min(self.policy.max_delay);
        jitter(base, self.policy.jitter_ratio)
    }
}

fn jitter(delay: Duration, ratio: u32) -> Duration {
    if ratio == 0 || delay.is_zero() {
        return delay;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |value| value.subsec_nanos());
    let width = ratio.saturating_mul(2).saturating_add(1);
    let offset = (nanos % width) as i64 - i64::from(ratio);
    let percent = (100_i64 + offset).max(0) as u32;
    delay.saturating_mul(percent) / 100
}
