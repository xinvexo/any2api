use std::time::Duration;

use any2api_domain::{
    OAuthAccountId, RequestAttemptFailureScope, RequestAttemptRetryDecision, RoutingCredentialId,
};
use tokio::time::timeout;

use super::{
    RetryExecution,
    decision::{RetryDecision, exclude_failed_path, retry_decision, telemetry_failure_scope},
};
use crate::{
    oauth::refresh::OAuthAuthenticationRefreshResult,
    public_request::{planning, response::FinalFailure, upstream::AttemptFailure},
};

impl RetryExecution<'_> {
    pub(super) async fn handle_failure(
        &mut self,
        attempt_no: u32,
        credential_id: RoutingCredentialId,
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
        let mut decision = retry_decision(&failure, &self.budget, credential_id, can_refresh_oauth);
        if let RetryDecision::OAuthRefresh {
            account_id,
            token_version,
        } = decision
        {
            self.oauth_refresh_attempted = true;
            if self.refresh_oauth(account_id, token_version).await? {
                self.previous_error = Some(public);
                self.services.recorder.annotate_attempt(
                    attempt_no,
                    failure_scope,
                    decision.telemetry(),
                );
                return Ok(());
            }
            decision = retry_decision(&failure, &self.budget, credential_id, false);
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
        let next_plan = planning::replan(
            next_snapshot.as_ref(),
            &self.plan,
            self.services.protocols.as_ref(),
            self.services.providers,
        )?;
        self.refreshed_oauth_target = refreshed_target;
        self.snapshot = next_snapshot;
        self.plan = next_plan;
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
        let delay = match decision {
            RetryDecision::Terminal => {
                self.services.recorder.annotate_attempt(
                    attempt_no,
                    failure_scope,
                    decision.telemetry(),
                );
                return Err(public);
            }
            RetryDecision::OAuthRefresh { .. } => {
                unreachable!("OAuth refresh is handled before retry execution")
            }
            RetryDecision::RetrySamePath(delay) => delay,
            RetryDecision::Reselect(scope) => {
                exclude_failed_path(&mut self.exclusions, &failure, scope);
                Duration::ZERO
            }
        };
        if delay >= self.budget.remaining() {
            self.services.recorder.annotate_attempt(
                attempt_no,
                failure_scope,
                RequestAttemptRetryDecision::Terminal,
            );
            return Err(public);
        }
        self.services
            .recorder
            .annotate_attempt(attempt_no, failure_scope, decision.telemetry());
        self.previous_error = Some(public);
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        Ok(())
    }
}
