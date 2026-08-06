use std::{future::Future, sync::Arc, time::Duration};

use any2api_domain::{
    ANY2API_UPSTREAM_TIMEOUT_MESSAGE, PublicError, PublicErrorCode, RequestAttemptFailureScope,
    RequestAttemptRetryDecision,
};
use any2api_protocol::api::ProtocolRegistry;
use any2api_provider::api::ProviderRegistry;
use any2api_transport::api::TransportManager;
use tokio::time::{Instant, timeout};

use super::{
    PublicResponse, affinity,
    planning::PlannedRequest,
    response::{FinalFailure, public_error},
    upstream,
};
use crate::{
    configuration::PublishedSnapshot,
    oauth::{
        OAuthQuotaActivity,
        refresh::{OAuthAuthenticationRefreshResult, OAuthRefresher},
    },
    request_telemetry::{AttemptTimeoutMarker, RequestRecorder},
    routing::CandidateExclusions,
};

use self::{
    budget::RetryBudget,
    decision::{RetryDecision, exclude_failed_path, retry_decision, telemetry_failure_scope},
};

mod budget;
mod decision;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(super) struct OAuthRetryServices<'a> {
    refresher: &'a OAuthRefresher,
    quota_activity: &'a OAuthQuotaActivity,
}

impl<'a> OAuthRetryServices<'a> {
    pub(super) const fn new(
        refresher: &'a OAuthRefresher,
        quota_activity: &'a OAuthQuotaActivity,
    ) -> Self {
        Self {
            refresher,
            quota_activity,
        }
    }
}

pub(super) async fn execute(
    mut snapshot: Arc<PublishedSnapshot>,
    protocols: Arc<ProtocolRegistry>,
    mut plan: PlannedRequest,
    providers: &ProviderRegistry,
    transport: &dyn TransportManager,
    oauth: Option<OAuthRetryServices<'_>>,
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
    let mut refreshed_oauth_target = None;
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
        let attempt_recorder =
            recorder.begin_attempt(attempt_no, &affinity.selected.candidate, affinity.bound);
        let timeout_marker = attempt_recorder.timeout_marker();
        let attempt_deadline = budget.deadline();
        let services = upstream::UpstreamServices {
            snapshot: snapshot.as_ref(),
            protocols: protocols.as_ref(),
            providers,
            transport,
            oauth_quota_activity: oauth.map(|services| services.quota_activity),
            attempt_deadline,
        };
        let attempt = if plan.decoded.stream {
            within_attempt_budget(
                attempt_deadline,
                timeout_marker,
                upstream::execute_stream_attempt(
                    services,
                    plan.decoded.as_ref(),
                    plan.public_model.clone(),
                    affinity,
                    attempt_recorder,
                    allow_credential_bound_headers,
                ),
            )
            .await
        } else {
            within_attempt_budget(
                attempt_deadline,
                timeout_marker,
                upstream::execute_buffered_attempt(
                    services,
                    plan.decoded.as_ref(),
                    &plan.public_model,
                    affinity,
                    attempt_recorder,
                    allow_credential_bound_headers,
                ),
            )
            .await
            .map(|attempt| attempt.map(PublicResponse::from))
        };
        let attempt = match attempt {
            Ok(attempt) => attempt,
            Err(error) => {
                recorder.annotate_attempt(
                    attempt_no,
                    Some(RequestAttemptFailureScope::Unattributed),
                    RequestAttemptRetryDecision::Terminal,
                );
                return Err(error);
            }
        };
        match attempt {
            Ok(response) => return Ok(response),
            Err(failure) => {
                if let Some((account_id, token_version)) = refreshed_oauth_target
                    && failure.oauth_authentication_target() == Some((account_id, token_version))
                    && let Some(refresher) = oauth.map(|services| services.refresher)
                {
                    refresher.record_refreshed_access_token_rejected(account_id, token_version);
                    refreshed_oauth_target = None;
                }
                let public = failure.final_failure();
                let can_refresh_oauth = !oauth_refresh_attempted
                    && oauth.is_some()
                    && failure.candidate().is_some_and(|candidate| {
                        budget.can_register_attempt(candidate.credential_id)
                    });
                let failure_scope = telemetry_failure_scope(&failure);
                let mut decision =
                    retry_decision(&failure, &budget, credential_id, can_refresh_oauth);
                if let RetryDecision::OAuthRefresh {
                    account_id,
                    token_version,
                } = decision
                {
                    oauth_refresh_attempted = true;
                    let refresher = oauth
                        .map(|services| services.refresher)
                        .expect("OAuth refresh decision requires a refresher");
                    let refreshed = timeout(
                        budget.remaining(),
                        refresher.refresh_after_authentication_failure(account_id, token_version),
                    )
                    .await
                    .ok()
                    .and_then(|result| match result {
                        OAuthAuthenticationRefreshResult::Refreshed(snapshot) => Some(snapshot),
                        OAuthAuthenticationRefreshResult::MissingRefreshToken(_)
                        | OAuthAuthenticationRefreshResult::PermanentlyRejected(_)
                        | OAuthAuthenticationRefreshResult::Failed(_) => None,
                    });
                    if let Some(next_snapshot) = refreshed {
                        refreshed_oauth_target = next_snapshot
                            .oauth_accounts()
                            .get(account_id)
                            .map(|account| (account_id, account.token_version()));
                        let next_plan = super::planning::replan(
                            next_snapshot.as_ref(),
                            &plan,
                            protocols.as_ref(),
                            providers,
                        )?;
                        snapshot = next_snapshot;
                        plan = next_plan;
                        previous_error = Some(public);
                        recorder.annotate_attempt(attempt_no, failure_scope, decision.telemetry());
                        continue;
                    }
                    decision = retry_decision(&failure, &budget, credential_id, false);
                }
                let delay = match decision {
                    RetryDecision::Terminal => {
                        recorder.annotate_attempt(attempt_no, failure_scope, decision.telemetry());
                        return Err(public);
                    }
                    RetryDecision::OAuthRefresh { .. } => {
                        unreachable!("OAuth refresh is handled before retry execution")
                    }
                    RetryDecision::RetrySamePath(delay) => delay,
                    RetryDecision::Reselect(scope) => {
                        exclude_failed_path(&mut exclusions, &failure, scope);
                        Duration::ZERO
                    }
                };
                if delay >= budget.remaining() {
                    recorder.annotate_attempt(
                        attempt_no,
                        failure_scope,
                        RequestAttemptRetryDecision::Terminal,
                    );
                    return Err(public);
                }
                recorder.annotate_attempt(attempt_no, failure_scope, decision.telemetry());
                previous_error = Some(public);
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
            }
        }
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
