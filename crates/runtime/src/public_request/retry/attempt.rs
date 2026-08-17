use any2api_domain::{RequestAttemptFailureScope, RequestAttemptRetryDecision};
use tokio::time::timeout;

use super::{RetryExecution, budget_exhausted, within_attempt_budget};
use crate::public_request::{
    PublicResponse, affinity,
    response::FinalFailure,
    upstream::{self, AttemptFailure},
};

pub(super) struct RegisteredAttempt {
    affinity: affinity::AffinitySelection,
    attempt_no: u32,
    allow_credential_bound_headers: bool,
}

pub(super) struct CompletedAttempt {
    pub(super) attempt_no: u32,
    pub(super) result: Result<PublicResponse, AttemptFailure>,
}

impl RetryExecution<'_> {
    pub(super) async fn select_attempt(&mut self) -> Result<RegisteredAttempt, FinalFailure> {
        let remaining = self.budget.remaining();
        if remaining.is_zero() {
            return Err(self.previous_or(|| budget_exhausted().into()));
        }
        let selection = {
            let credential_eligible =
                |credential_id| self.budget.can_register_attempt(credential_id);
            timeout(
                remaining,
                affinity::select(affinity::AffinitySelectionInput {
                    snapshot: self.snapshot.as_ref(),
                    operation: self.plan.decoded.operation,
                    affinity: &self.plan.decoded.affinity,
                    route_id: self.plan.route_id,
                    dialect: self.plan.dialect,
                    fallback_on_rate_limit: self.plan.fallback_on_rate_limit,
                    tiers: &self.plan.tiers,
                    selection_state: &self.selection_state,
                    credential_eligible: &credential_eligible,
                }),
            )
            .await
        };
        let affinity = match selection {
            Ok(Ok(selection)) => selection,
            Ok(Err(error)) => return Err(self.previous_or(|| error.into())),
            Err(_) => return Err(self.previous_or(|| budget_exhausted().into())),
        };
        let credential_id = affinity.selected.candidate.credential_id;
        let Some(attempt_no) = self.budget.register_attempt(&affinity.selected.candidate) else {
            affinity.selected.rollback_before_attempt();
            return Err(self.previous_or(|| budget_exhausted().into()));
        };
        let allow_credential_bound_headers = match self.credential_bound_header_owner {
            Some(owner) => owner == credential_id,
            None => {
                self.credential_bound_header_owner = Some(credential_id);
                true
            }
        };
        Ok(RegisteredAttempt {
            affinity,
            attempt_no,
            allow_credential_bound_headers,
        })
    }

    pub(super) async fn execute_attempt(
        &mut self,
        registered: RegisteredAttempt,
    ) -> Result<CompletedAttempt, FinalFailure> {
        let RegisteredAttempt {
            affinity,
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
                return Err(self.previous_or(|| error));
            }
        };
        Ok(CompletedAttempt { attempt_no, result })
    }

    fn previous_or(&mut self, fallback: impl FnOnce() -> FinalFailure) -> FinalFailure {
        take_previous_or(&mut self.previous_error, fallback)
    }
}

fn take_previous_or(
    previous: &mut Option<FinalFailure>,
    fallback: impl FnOnce() -> FinalFailure,
) -> FinalFailure {
    previous.take().unwrap_or_else(fallback)
}

#[cfg(test)]
mod tests {
    use any2api_domain::{
        ErrorClass, PublicErrorCode, RetrySafety, UpstreamError, UpstreamErrorClassification,
        UpstreamErrorKind,
    };
    use bytes::Bytes;
    use http::{HeaderMap, HeaderValue, StatusCode};

    use super::{super::budget_exhausted, take_previous_or};
    use crate::public_request::response::FinalFailure;

    #[test]
    fn previous_upstream_failure_wins_over_a_later_local_timeout() {
        let mut headers = HeaderMap::new();
        headers.insert("x-upstream-attempt", HeaderValue::from_static("first"));
        let upstream = UpstreamError::new(
            UpstreamErrorClassification::new(
                UpstreamErrorKind::Transient,
                RetrySafety::Ambiguous,
                None,
            ),
            Some("first upstream failed".to_owned()),
        );
        let mut previous = Some(FinalFailure::upstream(
            headers,
            StatusCode::SERVICE_UNAVAILABLE,
            Bytes::from_static(br#"{"error":"first"}"#),
            &upstream,
        ));

        let selected = take_previous_or(&mut previous, || {
            panic!("a later local timeout must not replace a real upstream response")
        });
        let FinalFailure::Upstream {
            response,
            error_class,
            error_message,
        } = selected
        else {
            panic!("the previous upstream failure must be returned")
        };
        assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers["x-upstream-attempt"], "first");
        assert_eq!(response.body, br#"{"error":"first"}"#.as_slice());
        assert_eq!(error_class, ErrorClass::Upstream);
        assert_eq!(error_message.as_deref(), Some("first upstream failed"));
        assert!(previous.is_none());
    }

    #[test]
    fn local_timeout_is_preserved_when_no_upstream_failure_exists() {
        let mut previous = None;
        let selected = take_previous_or(&mut previous, || budget_exhausted().into());

        let FinalFailure::Local { error } = selected else {
            panic!("the local timeout must remain the final failure")
        };
        assert_eq!(error.code(), PublicErrorCode::GatewayTimeout);
    }
}
