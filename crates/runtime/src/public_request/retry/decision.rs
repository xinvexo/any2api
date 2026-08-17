use std::time::Duration;

use any2api_domain::{
    OAuthAccountId, RequestAttemptFailureScope, RequestAttemptRetryDecision, UpstreamErrorKind,
    UpstreamFailureAttribution,
};
use any2api_protocol::api::ProtocolRetryDelayBasis;
use any2api_transport::api::TransportFailureScope;
use tokio::time::Instant;

use super::budget::RetryBudget;
use crate::{
    public_request::upstream::AttemptFailure,
    routing::{CandidateFailureScope, CandidateSelectionState},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RetryDecision {
    Terminal,
    OAuthRefresh {
        account_id: OAuthAccountId,
        token_version: u64,
    },
    RetrySamePath(Duration),
    PreferAlternate {
        scope: CandidateFailureScope,
        retry_delay: Duration,
    },
    Reselect {
        scope: CandidateFailureScope,
    },
}

impl RetryDecision {
    pub(super) const fn telemetry(self) -> RequestAttemptRetryDecision {
        match self {
            Self::Terminal => RequestAttemptRetryDecision::Terminal,
            Self::OAuthRefresh { .. } => RequestAttemptRetryDecision::OAuthRefresh,
            Self::RetrySamePath(_) => RequestAttemptRetryDecision::RetrySamePath,
            Self::PreferAlternate { .. } | Self::Reselect { .. } => {
                RequestAttemptRetryDecision::Reselect
            }
        }
    }

    pub(super) const fn required_wait(self) -> Option<Duration> {
        match self {
            Self::RetrySamePath(delay) => Some(delay),
            Self::Terminal
            | Self::OAuthRefresh { .. }
            | Self::PreferAlternate { .. }
            | Self::Reselect { .. } => None,
        }
    }
}

pub(super) fn retry_decision(
    failure: &AttemptFailure,
    budget: &RetryBudget,
    can_refresh_oauth: bool,
) -> RetryDecision {
    if !budget.can_retry() {
        return RetryDecision::Terminal;
    }
    match failure {
        AttemptFailure::Public(_) => RetryDecision::Terminal,
        AttemptFailure::Transport { .. } => {
            if !failure.retry_safety().allows_automatic_retry() {
                return RetryDecision::Terminal;
            }
            let candidate = failure
                .candidate()
                .expect("transport failure always identifies its candidate");
            let delay = budget.next_delay(candidate, failure.retry_after());
            if failure.bound() {
                RetryDecision::RetrySamePath(delay)
            } else {
                RetryDecision::Reselect {
                    scope: failure_scope(failure),
                }
            }
        }
        AttemptFailure::Upstream { error, .. } => {
            if matches!(
                failure.upstream_origin(),
                Some(
                    crate::public_request::upstream::UpstreamFailureOrigin::HttpStatus {
                        error_body_complete: false,
                    }
                )
            ) {
                return RetryDecision::Terminal;
            }
            let classification = error.classification();
            let kind = classification.kind();
            if kind == UpstreamErrorKind::InvalidRequest {
                return RetryDecision::Terminal;
            }
            if can_refresh_oauth
                && classification.retry_safety().allows_automatic_retry()
                && let Some((account_id, token_version)) = failure.oauth_authentication_target()
            {
                return RetryDecision::OAuthRefresh {
                    account_id,
                    token_version,
                };
            }
            if failure.bound() {
                if !classification.retry_safety().allows_automatic_retry()
                    || is_nonrecovering_bound_kind(kind)
                {
                    return RetryDecision::Terminal;
                }
                return RetryDecision::RetrySamePath(retry_delay(failure, budget));
            }
            let scope = failure_scope(failure);
            if is_hard_reselection_kind(kind) {
                RetryDecision::Reselect { scope }
            } else {
                RetryDecision::PreferAlternate {
                    scope,
                    retry_delay: retry_delay(failure, budget),
                }
            }
        }
    }
}

fn retry_delay(failure: &AttemptFailure, budget: &RetryBudget) -> Duration {
    match failure
        .upstream_origin()
        .map(|origin| origin.retry_delay_basis())
        .unwrap_or(ProtocolRetryDelayBasis::CandidateAttempts)
    {
        ProtocolRetryDelayBasis::CandidateAttempts => budget.next_delay(
            failure
                .candidate()
                .expect("upstream failure always identifies its candidate"),
            failure.retry_after(),
        ),
        ProtocolRetryDelayBasis::RequestAttempts => {
            budget.next_request_delay(failure.retry_after())
        }
    }
}

const fn is_hard_reselection_kind(kind: UpstreamErrorKind) -> bool {
    matches!(
        kind,
        UpstreamErrorKind::Authentication
            | UpstreamErrorKind::PermissionDenied
            | UpstreamErrorKind::QuotaExhausted
            | UpstreamErrorKind::ModelUnavailable
            | UpstreamErrorKind::OperationUnavailable
    )
}

const fn is_nonrecovering_bound_kind(kind: UpstreamErrorKind) -> bool {
    is_hard_reselection_kind(kind)
}

fn failure_scope(failure: &AttemptFailure) -> CandidateFailureScope {
    match failure {
        AttemptFailure::Transport { error, .. } => match error.failure_scope {
            TransportFailureScope::Endpoint => CandidateFailureScope::Endpoint,
            TransportFailureScope::Proxy => CandidateFailureScope::Proxy,
            TransportFailureScope::EgressPath => CandidateFailureScope::EgressPath,
            TransportFailureScope::Unattributed => CandidateFailureScope::ExactCandidate,
        },
        AttemptFailure::Upstream { error, .. } => match error.classification().attribution() {
            UpstreamFailureAttribution::Authentication | UpstreamFailureAttribution::Credential => {
                CandidateFailureScope::Credential
            }
            UpstreamFailureAttribution::CredentialModel => CandidateFailureScope::CredentialModel,
            UpstreamFailureAttribution::RouteOperation => CandidateFailureScope::RouteOperation,
            UpstreamFailureAttribution::Endpoint => CandidateFailureScope::Endpoint,
            UpstreamFailureAttribution::EgressPath => CandidateFailureScope::EgressPath,
            UpstreamFailureAttribution::Unattributed => CandidateFailureScope::ExactCandidate,
        },
        AttemptFailure::Public(_) => CandidateFailureScope::ExactCandidate,
    }
}

pub(super) fn telemetry_failure_scope(
    failure: &AttemptFailure,
) -> Option<RequestAttemptFailureScope> {
    Some(match failure {
        AttemptFailure::Transport { error, .. } => match error.failure_scope {
            TransportFailureScope::Endpoint => RequestAttemptFailureScope::Endpoint,
            TransportFailureScope::Proxy => RequestAttemptFailureScope::Proxy,
            TransportFailureScope::EgressPath => RequestAttemptFailureScope::EgressPath,
            TransportFailureScope::Unattributed => RequestAttemptFailureScope::ExactCandidate,
        },
        AttemptFailure::Upstream { error, .. } => match error.classification().attribution() {
            UpstreamFailureAttribution::Unattributed => RequestAttemptFailureScope::ExactCandidate,
            UpstreamFailureAttribution::Authentication => {
                RequestAttemptFailureScope::Authentication
            }
            UpstreamFailureAttribution::Credential => RequestAttemptFailureScope::Credential,
            UpstreamFailureAttribution::CredentialModel => {
                RequestAttemptFailureScope::CredentialModel
            }
            UpstreamFailureAttribution::RouteOperation => {
                RequestAttemptFailureScope::RouteOperation
            }
            UpstreamFailureAttribution::EgressPath => RequestAttemptFailureScope::EgressPath,
            UpstreamFailureAttribution::Endpoint => RequestAttemptFailureScope::Endpoint,
        },
        AttemptFailure::Public(_) => return None,
    })
}

pub(super) fn apply_candidate_recovery(
    state: &mut CandidateSelectionState,
    failure: &AttemptFailure,
    decision: RetryDecision,
    now: Instant,
) {
    let Some(candidate) = failure.candidate() else {
        return;
    };
    match decision {
        RetryDecision::PreferAlternate { scope, retry_delay } => {
            state.note_failed(candidate);
            if let Some(not_before) = now.checked_add(retry_delay) {
                state.defer(candidate, scope, not_before);
            } else {
                state.exclude(candidate, scope);
            }
        }
        RetryDecision::Reselect { scope } => {
            state.note_failed(candidate);
            state.exclude(candidate, scope);
        }
        RetryDecision::Terminal
        | RetryDecision::OAuthRefresh { .. }
        | RetryDecision::RetrySamePath(_) => {}
    }
}
