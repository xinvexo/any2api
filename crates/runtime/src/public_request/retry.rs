use std::{future::Future, sync::Arc};

use any2api_domain::{
    ANY2API_UPSTREAM_TIMEOUT_MESSAGE, OAuthAccountId, PublicError, PublicErrorCode,
    RoutingCredentialId,
};
use any2api_protocol::api::ProtocolRegistry;
use any2api_provider::api::ProviderRegistry;
use any2api_transport::api::TransportManager;
use tokio::time::Instant;

use super::{
    PublicResponse,
    live_routing::LiveRoutingSnapshots,
    planning::{self, PlannedRequest, RoutingReplanMode},
    response::{FinalFailure, public_error},
};
use crate::{
    configuration::PublishedSnapshot,
    oauth::{OAuthQuotaActivity, refresh::OAuthRefresher},
    public_request::selection::SelectionWaitState,
    request_telemetry::{AttemptTimeoutMarker, RequestRecorder},
    routing::CandidateSelectionState,
};

use self::{attempt::CompletedAttempt, budget::RetryBudget};

mod attempt;
mod budget;
mod decision;
mod failure;
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

struct RetryServices<'a> {
    protocols: Arc<ProtocolRegistry>,
    providers: &'a ProviderRegistry,
    transport: &'a dyn TransportManager,
    oauth: Option<OAuthRetryServices<'a>>,
    recorder: RequestRecorder,
}

pub(super) struct RetryExecutionInput<'a> {
    pub(super) policy_snapshot: Arc<PublishedSnapshot>,
    pub(super) routing_snapshot: Arc<PublishedSnapshot>,
    pub(super) protocols: Arc<ProtocolRegistry>,
    pub(super) plan: PlannedRequest,
    pub(super) replan_mode: RoutingReplanMode,
    pub(super) providers: &'a ProviderRegistry,
    pub(super) transport: &'a dyn TransportManager,
    pub(super) live: LiveRoutingSnapshots,
    pub(super) oauth: Option<OAuthRetryServices<'a>>,
    pub(super) recorder: RequestRecorder,
}

struct RetryExecution<'a> {
    policy_snapshot: Arc<PublishedSnapshot>,
    routing_snapshot: Arc<PublishedSnapshot>,
    plan: PlannedRequest,
    replan_mode: RoutingReplanMode,
    live: LiveRoutingSnapshots,
    services: RetryServices<'a>,
    budget: RetryBudget,
    selection_state: CandidateSelectionState,
    selection_wait: SelectionWaitState,
    previous_error: Option<FinalFailure>,
    oauth_refresh_attempted: bool,
    refreshed_oauth_target: Option<(OAuthAccountId, u64)>,
    credential_bound_header_owner: Option<RoutingCredentialId>,
}

pub(super) async fn execute(
    input: RetryExecutionInput<'_>,
) -> Result<PublicResponse, FinalFailure> {
    let RetryExecutionInput {
        policy_snapshot,
        routing_snapshot,
        protocols,
        plan,
        replan_mode,
        providers,
        transport,
        live,
        oauth,
        recorder,
    } = input;
    let budget = RetryBudget::new(
        policy_snapshot.reliability_policy(),
        plan.decoded.operation,
        plan.decoded.execution_profile,
    );
    RetryExecution {
        policy_snapshot,
        routing_snapshot,
        plan,
        replan_mode,
        live,
        services: RetryServices {
            protocols,
            providers,
            transport,
            oauth,
            recorder,
        },
        budget,
        selection_state: CandidateSelectionState::default(),
        selection_wait: SelectionWaitState::default(),
        previous_error: None,
        oauth_refresh_attempted: false,
        refreshed_oauth_target: None,
        credential_bound_header_owner: None,
    }
    .run()
    .await
}

impl RetryExecution<'_> {
    pub(super) fn rebase_to(
        &mut self,
        next_snapshot: Arc<PublishedSnapshot>,
    ) -> Result<(), PublicError> {
        self.live.validate(next_snapshot.as_ref())?;
        let next_plan = planning::replan(
            next_snapshot.as_ref(),
            &self.plan,
            self.replan_mode,
            self.policy_snapshot.queue_policy().fallback_on_rate_limit(),
            self.services.protocols.as_ref(),
            self.services.providers,
        )?;
        self.routing_snapshot = next_snapshot;
        self.plan = next_plan;
        Ok(())
    }
}

impl RetryExecution<'_> {
    async fn run(mut self) -> Result<PublicResponse, FinalFailure> {
        if let Some(latest) = self.live.refresh(self.routing_snapshot.as_ref()) {
            self.rebase_to(latest).map_err(FinalFailure::from)?;
        }
        loop {
            let selected = self.select_attempt().await?;
            let AttemptExecution::Completed(completed) = self.execute_attempt(selected).await?
            else {
                continue;
            };
            match completed.result {
                Ok(response) => return Ok(response),
                Err(failure) => {
                    self.handle_failure(completed.attempt_no, failure).await?;
                }
            }
        }
    }
}

enum AttemptExecution {
    Completed(CompletedAttempt),
    Reselect,
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
