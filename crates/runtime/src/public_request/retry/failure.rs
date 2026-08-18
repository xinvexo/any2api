use std::time::Duration;

use any2api_domain::{OAuthAccountId, RequestAttemptFailureScope, RequestAttemptRetryDecision};
use tokio::time::{Instant, timeout};

use super::{
    RetryExecution,
    decision::{RetryDecision, apply_candidate_recovery, retry_decision, telemetry_failure_scope},
};
use crate::{
    oauth::refresh::OAuthAuthenticationRefreshResult,
    public_request::{response::FinalFailure, upstream::AttemptFailure},
};

impl RetryExecution<'_> {
    pub(super) async fn handle_failure(
        &mut self,
        attempt_no: u32,
        failure: AttemptFailure,
    ) -> Result<(), FinalFailure> {
        self.note_rejected_refreshed_token(&failure);
        let public = failure.final_failure();
        let can_refresh_oauth = !self.oauth_refresh_attempted
            && self.services.oauth.is_some()
            && failure
                .candidate()
                .is_some_and(|candidate| self.budget.can_register_attempt(candidate.credential_id));
        let failure_scope = telemetry_failure_scope(&failure);
        let mut decision = retry_decision(&failure, &self.budget, can_refresh_oauth);
        if decision
            .required_wait()
            .is_some_and(|delay| !self.budget.can_wait(delay))
        {
            decision = RetryDecision::Terminal;
        }
        if let RetryDecision::OAuthRefresh {
            account_id,
            token_version,
        } = decision
        {
            self.oauth_refresh_attempted = true;
            if self.refresh_oauth(account_id, token_version).await? {
                return self
                    .apply_retry_decision(attempt_no, failure_scope, failure, public, decision)
                    .await;
            }
            decision = retry_decision(&failure, &self.budget, false);
        }
        self.apply_retry_decision(attempt_no, failure_scope, failure, public, decision)
            .await
    }

    fn note_rejected_refreshed_token(&mut self, failure: &AttemptFailure) {
        if let Some((account_id, token_version)) = self.refreshed_oauth_target
            && failure.oauth_authentication_target() == Some((account_id, token_version))
            && let Some(refresher) = self.services.oauth.map(|services| services.refresher)
        {
            refresher.record_refreshed_access_token_rejected(account_id, token_version);
            self.refreshed_oauth_target = None;
        }
    }

    async fn refresh_oauth(
        &mut self,
        account_id: OAuthAccountId,
        token_version: u64,
    ) -> Result<bool, FinalFailure> {
        let refresher = self
            .services
            .oauth
            .map(|services| services.refresher)
            .expect("OAuth refresh decision requires a refresher");
        let refreshed = timeout(
            self.budget.remaining(),
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
        let Some(next_snapshot) = refreshed else {
            return Ok(false);
        };
        let refreshed_target = next_snapshot
            .oauth_accounts()
            .get(account_id)
            .map(|account| (account_id, account.token_version()));
        self.rebase_to(next_snapshot).map_err(FinalFailure::from)?;
        self.refreshed_oauth_target = refreshed_target;
        Ok(true)
    }

    async fn apply_retry_decision(
        &mut self,
        attempt_no: u32,
        failure_scope: Option<RequestAttemptFailureScope>,
        failure: AttemptFailure,
        public: FinalFailure,
        decision: RetryDecision,
    ) -> Result<(), FinalFailure> {
        match decision {
            RetryDecision::Terminal => {
                self.services.recorder.annotate_attempt(
                    attempt_no,
                    failure_scope,
                    RequestAttemptRetryDecision::Terminal,
                );
                return Err(public);
            }
            RetryDecision::RetrySamePath(delay) => {
                if !self.budget.can_wait(delay) {
                    self.services.recorder.annotate_attempt(
                        attempt_no,
                        failure_scope,
                        RequestAttemptRetryDecision::Terminal,
                    );
                    return Err(public);
                }
                wait_retry_delay(delay).await;
            }
            RetryDecision::PreferAlternate { .. } | RetryDecision::Reselect { .. } => {
                apply_candidate_recovery(
                    &mut self.selection_state,
                    &failure,
                    decision,
                    Instant::now(),
                );
            }
            RetryDecision::OAuthRefresh { .. } => {}
        }
        self.services
            .recorder
            .annotate_attempt(attempt_no, failure_scope, decision.telemetry());
        self.previous_error = Some(public);
        Ok(())
    }
}

async fn wait_retry_delay(delay: Duration) {
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }
}

#[cfg(test)]
mod tests {
    use super::wait_retry_delay;
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn retry_wait_cannot_complete_before_the_semantic_delay() {
        let (completed_tx, mut completed_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            wait_retry_delay(Duration::from_secs(5)).await;
            completed_tx.send(()).ok();
        });
        tokio::task::yield_now().await;
        assert!(matches!(
            completed_rx.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));

        tokio::time::advance(Duration::from_secs(4)).await;
        tokio::task::yield_now().await;
        assert!(matches!(
            completed_rx.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));

        tokio::time::advance(Duration::from_secs(1)).await;
        completed_rx.await.expect("retry delay completion");
    }
}
