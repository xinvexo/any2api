use any2api_domain::{
    RequestAttemptFailureScope, RequestAttemptRetryDecision, RoutingCredentialId,
};
use tokio::time::timeout;

use super::{RetryExecution, budget_exhausted, within_attempt_budget};
use crate::public_request::{
    PublicResponse, affinity,
    response::FinalFailure,
    upstream::{self, AttemptFailure},
};

pub(super) struct RegisteredAttempt {
    affinity: affinity::AffinitySelection,
    credential_id: RoutingCredentialId,
    attempt_no: u32,
    allow_credential_bound_headers: bool,
}

pub(super) struct CompletedAttempt {
    pub(super) attempt_no: u32,
    pub(super) credential_id: RoutingCredentialId,
    pub(super) result: Result<PublicResponse, AttemptFailure>,
}

impl RetryExecution<'_> {
    pub(super) async fn select_attempt(&mut self) -> Result<RegisteredAttempt, FinalFailure> {
        let remaining = self.budget.remaining();
        if remaining.is_zero() {
            return Err(self.previous_or(|| budget_exhausted().into()));
        }
        let selection = timeout(
            remaining,
            affinity::select(affinity::AffinitySelectionInput {
                snapshot: self.snapshot.as_ref(),
                operation: self.plan.decoded.operation,
                affinity: &self.plan.decoded.affinity,
                route_id: self.plan.route_id,
                dialect: self.plan.dialect,
                fallback_on_rate_limit: self.plan.fallback_on_rate_limit,
                tiers: &self.plan.tiers,
                exclusions: &self.exclusions,
            }),
        )
        .await;
        let affinity = match selection {
            Ok(Ok(selection)) => selection,
            Ok(Err(error)) => return Err(self.previous_or(|| error.into())),
            Err(_) => return Err(self.previous_or(|| budget_exhausted().into())),
        };
        let credential_id = affinity.selected.candidate.credential_id;
        let allow_credential_bound_headers = match self.credential_bound_header_owner {
            Some(owner) => owner == credential_id,
            None => {
                self.credential_bound_header_owner = Some(credential_id);
                true
            }
        };
        let Some(attempt_no) = self.budget.register_attempt(credential_id) else {
            return Err(self.previous_or(|| budget_exhausted().into()));
        };
        Ok(RegisteredAttempt {
            affinity,
            credential_id,
            attempt_no,
            allow_credential_bound_headers,
        })
    }

    pub(super) async fn execute_attempt(
        &self,
        registered: RegisteredAttempt,
    ) -> Result<CompletedAttempt, FinalFailure> {
        let RegisteredAttempt {
            affinity,
            credential_id,
            attempt_no,
            allow_credential_bound_headers,
        } = registered;
        let attempt_recorder = self.services.recorder.begin_attempt(
            attempt_no,
            &affinity.selected.candidate,
            affinity.bound,
        );
        let timeout_marker = attempt_recorder.timeout_marker();
        let attempt_deadline = self.budget.deadline();
        let services = upstream::UpstreamServices {
            snapshot: self.snapshot.as_ref(),
            protocols: self.services.protocols.as_ref(),
            providers: self.services.providers,
            transport: self.services.transport,
            oauth_quota_activity: self.services.oauth.map(|services| services.quota_activity),
            attempt_deadline,
        };
        let attempt = if self.plan.decoded.stream {
            within_attempt_budget(
                attempt_deadline,
                timeout_marker,
                upstream::execute_stream_attempt(
                    services,
                    self.plan.decoded.as_ref(),
                    self.plan.public_model.clone(),
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
                    self.plan.decoded.as_ref(),
                    &self.plan.public_model,
                    affinity,
                    attempt_recorder,
                    allow_credential_bound_headers,
                ),
            )
            .await
            .map(|attempt| attempt.map(PublicResponse::from))
        };
        let result = match attempt {
            Ok(attempt) => attempt,
            Err(error) => {
                self.services.recorder.annotate_attempt(
                    attempt_no,
                    Some(RequestAttemptFailureScope::Unattributed),
                    RequestAttemptRetryDecision::Terminal,
                );
                return Err(error);
            }
        };
        Ok(CompletedAttempt {
            attempt_no,
            credential_id,
            result,
        })
    }

    fn previous_or(&mut self, fallback: impl FnOnce() -> FinalFailure) -> FinalFailure {
        self.previous_error.take().unwrap_or_else(fallback)
    }
}
