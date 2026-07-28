use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};

use any2api_domain::{
    ANY2API_UPSTREAM_TIMEOUT_MESSAGE, PublicError, PublicErrorCode, RoutingCredentialId,
    UpstreamErrorKind,
};
use any2api_protocol::api::ProtocolRegistry;
use any2api_provider::api::ProviderRegistry;
use any2api_transport::api::{TransportFailureScope, TransportManager};
use tokio::time::{Instant, timeout};

use super::{
    PublicResponse, affinity, execution_limits,
    planning::PlannedRequest,
    response::{FinalFailure, public_error},
    upstream::{self, AttemptFailure},
};
use crate::{
    configuration::PublishedSnapshot,
    health::ReliabilityPolicy,
    oauth::refresh::OAuthRefresher,
    request_telemetry::{AttemptTimeoutMarker, RequestRecorder},
    routing::CandidateExclusions,
};

pub(super) async fn execute(
    mut snapshot: Arc<PublishedSnapshot>,
    protocols: Arc<ProtocolRegistry>,
    mut plan: PlannedRequest,
    providers: &ProviderRegistry,
    transport: &dyn TransportManager,
    oauth_refresher: Option<&OAuthRefresher>,
    recorder: RequestRecorder,
) -> Result<PublicResponse, FinalFailure> {
    let policy = snapshot.reliability_policy();
    let mut budget = RetryBudget::new(
        policy,
        plan.decoded.operation,
        plan.decoded.execution_profile,
    );
    let mut exclusions = CandidateExclusions::default();
    let mut previous_error = None;
    let mut oauth_refresh_attempted = false;
    let mut credential_bound_header_owner = None;

    loop {
        let remaining = budget.remaining();
        if remaining.is_zero() {
            return Err(previous_error.unwrap_or_else(|| budget_exhausted().into()));
        }
        let selection = timeout(
            remaining,
            affinity::select(affinity::AffinitySelectionInput {
                snapshot: snapshot.as_ref(),
                operation: plan.decoded.operation,
                affinity: &plan.decoded.affinity,
                route_id: plan.route_id,
                dialect: plan.dialect,
                fallback_on_rate_limit: plan.fallback_on_rate_limit,
                tiers: &plan.tiers,
                exclusions: &exclusions,
            }),
        )
        .await;
        let affinity = match selection {
            Ok(Ok(selection)) => selection,
            Ok(Err(error)) => return Err(previous_error.unwrap_or_else(|| error.into())),
            Err(_) => {
                return Err(previous_error.unwrap_or_else(|| budget_exhausted().into()));
            }
        };
        let credential_id = affinity.selected.candidate.credential_id;
        let allow_credential_bound_headers = match credential_bound_header_owner {
            Some(owner) => owner == credential_id,
            None => {
                credential_bound_header_owner = Some(credential_id);
                true
            }
        };
        let Some(attempt_no) = budget.register_attempt(credential_id) else {
            return Err(previous_error.unwrap_or_else(|| budget_exhausted().into()));
        };
        let attempt_recorder = recorder.begin_attempt(attempt_no, &affinity.selected.candidate);
        let timeout_marker = attempt_recorder.timeout_marker();
        let attempt_deadline = budget.deadline();
        let services = upstream::UpstreamServices {
            snapshot: snapshot.as_ref(),
            protocols: protocols.as_ref(),
            providers,
            transport,
            attempt_deadline,
        };
        let attempt = if plan.decoded.stream {
            within_attempt_budget(
                attempt_deadline,
                timeout_marker,
                upstream::execute_stream_attempt(
                    services,
                    plan.decoded.clone(),
                    plan.public_model.clone(),
                    affinity,
                    attempt_recorder,
                    allow_credential_bound_headers,
                ),
            )
            .await?
        } else {
            within_attempt_budget(
                attempt_deadline,
                timeout_marker,
                upstream::execute_buffered_attempt(
                    services,
                    plan.decoded.clone(),
                    &plan.public_model,
                    affinity,
                    attempt_recorder,
                    allow_credential_bound_headers,
                ),
            )
            .await?
            .map(PublicResponse::from)
        };
        match attempt {
            Ok(response) => return Ok(response),
            Err(failure) => {
                let public = failure.final_failure();
                if !oauth_refresh_attempted
                    && let Some(refresher) = oauth_refresher
                    && let Some((account_id, token_version)) = failure.oauth_authentication_target()
                    && let Some(candidate) = failure.candidate()
                    && budget.can_register_attempt(candidate.credential_id)
                {
                    oauth_refresh_attempted = true;
                    let refreshed = timeout(
                        budget.remaining(),
                        refresher.refresh_after_authentication_failure(account_id, token_version),
                    )
                    .await
                    .ok()
                    .flatten();
                    if let Some(next_snapshot) = refreshed {
                        let next_plan = super::planning::replan(
                            next_snapshot.as_ref(),
                            &plan,
                            protocols.as_ref(),
                            providers,
                        )?;
                        snapshot = next_snapshot;
                        plan = next_plan;
                        previous_error = Some(public);
                        continue;
                    }
                }
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
        AttemptFailure::Upstream { error, .. } => match error.classification().kind() {
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
        PublicErrorCode::GatewayTimeout,
        ANY2API_UPSTREAM_TIMEOUT_MESSAGE,
    )
}

async fn within_attempt_budget<T>(
    deadline: Instant,
    timeout_marker: AttemptTimeoutMarker,
    future: impl Future<Output = T>,
) -> Result<T, FinalFailure> {
    let deadline = tokio::time::sleep_until(deadline);
    tokio::pin!(deadline);
    tokio::pin!(future);
    tokio::select! {
        biased;
        result = &mut future => Ok(result),
        _ = &mut deadline => {
            timeout_marker.mark_timed_out();
            Err(budget_exhausted().into())
        }
    }
}

struct RetryBudget {
    policy: ReliabilityPolicy,
    deadline: Instant,
    attempts: u32,
    switches: u32,
    last_credential: Option<RoutingCredentialId>,
    attempts_by_credential: HashMap<RoutingCredentialId, u32>,
}

impl RetryBudget {
    fn new(
        policy: ReliabilityPolicy,
        operation: any2api_domain::ProtocolOperation,
        profile: any2api_protocol::api::RequestExecutionProfile,
    ) -> Self {
        Self {
            policy,
            deadline: Instant::now()
                + execution_limits::retry_budget(operation, profile, policy.precommit_total_budget),
            attempts: 0,
            switches: 0,
            last_credential: None,
            attempts_by_credential: HashMap::new(),
        }
    }

    fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    fn deadline(&self) -> Instant {
        self.deadline
    }

    fn register_attempt(&mut self, credential_id: RoutingCredentialId) -> Option<u32> {
        if !self.can_register_attempt(credential_id) {
            return None;
        }
        if self
            .last_credential
            .is_some_and(|previous| previous != credential_id)
        {
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

    fn can_register_attempt(&self, credential_id: RoutingCredentialId) -> bool {
        if self.attempts >= self.policy.max_total_attempts || self.remaining().is_zero() {
            return false;
        }
        if self
            .last_credential
            .is_some_and(|previous| previous != credential_id)
            && self.switches >= self.policy.max_credential_switches
        {
            return false;
        }
        self.attempts_by_credential
            .get(&credential_id)
            .copied()
            .unwrap_or(0)
            <= self.policy.max_same_credential_retries
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

/// Retry jitter only needs de-synchronization, not unpredictability, so the
/// wall-clock sub-second nanos stand in for a real RNG dependency.
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::within_attempt_budget;
    use crate::request_telemetry::AttemptRecorder;

    #[tokio::test(start_paused = true)]
    async fn attempt_result_at_the_deadline_wins_over_the_outer_timeout() {
        let deadline = Instant::now() + Duration::from_millis(25);
        let marker = AttemptRecorder::disabled().timeout_marker();
        let attempt = async {
            tokio::time::sleep_until(deadline).await;
            7
        };

        match within_attempt_budget(deadline, marker, attempt).await {
            Ok(value) => assert_eq!(value, 7),
            Err(_) => panic!("the completed attempt must win at the shared deadline"),
        }
    }
}
