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
    planning::PlannedRequest,
    response::{FinalFailure, public_error},
};
use crate::{
    configuration::PublishedSnapshot,
    oauth::{OAuthQuotaActivity, refresh::OAuthRefresher},
    request_telemetry::{AttemptTimeoutMarker, RequestRecorder},
    routing::CandidateSelectionState,
};

use self::budget::RetryBudget;

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

struct RetryExecution<'a> {
    snapshot: Arc<PublishedSnapshot>,
    plan: PlannedRequest,
    services: RetryServices<'a>,
    budget: RetryBudget,
    selection_state: CandidateSelectionState,
    previous_error: Option<FinalFailure>,
    oauth_refresh_attempted: bool,
    refreshed_oauth_target: Option<(OAuthAccountId, u64)>,
    credential_bound_header_owner: Option<RoutingCredentialId>,
}

pub(super) async fn execute(
    snapshot: Arc<PublishedSnapshot>,
    protocols: Arc<ProtocolRegistry>,
    plan: PlannedRequest,
    providers: &ProviderRegistry,
    transport: &dyn TransportManager,
    oauth: Option<OAuthRetryServices<'_>>,
    recorder: RequestRecorder,
) -> Result<PublicResponse, FinalFailure> {
    let budget = RetryBudget::new(
        snapshot.reliability_policy(),
        plan.decoded.operation,
        plan.decoded.execution_profile,
    );
    RetryExecution {
        snapshot,
        plan,
        services: RetryServices {
            protocols,
            providers,
            transport,
            oauth,
            recorder,
        },
        budget,
        selection_state: CandidateSelectionState::default(),
        previous_error: None,
        oauth_refresh_attempted: false,
        refreshed_oauth_target: None,
        credential_bound_header_owner: None,
    }
    .run()
    .await
}

impl RetryExecution<'_> {
    async fn run(mut self) -> Result<PublicResponse, FinalFailure> {
        loop {
            let selected = self.select_attempt().await?;
            let completed = self.execute_attempt(selected).await?;
            match completed.result {
                Ok(response) => return Ok(response),
                Err(failure) => {
                    self.handle_failure(completed.attempt_no, failure).await?;
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
