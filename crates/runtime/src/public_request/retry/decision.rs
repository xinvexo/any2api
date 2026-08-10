use std::time::Duration;

use any2api_domain::{
    OAuthAccountId, RequestAttemptFailureScope, RequestAttemptRetryDecision, RoutingCredentialId,
    UpstreamErrorKind, UpstreamFailureAttribution,
};
use any2api_protocol::api::StreamRetryReason;
use any2api_transport::api::TransportFailureScope;

use super::budget::RetryBudget;
use crate::{public_request::upstream::AttemptFailure, routing::CandidateExclusions};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RetryDecision {
    Terminal,
    OAuthRefresh {
        account_id: OAuthAccountId,
        token_version: u64,
        delay: Duration,
    },
    RetrySamePath(Duration),
    Reselect {
        exclusion: RetryExclusion,
        delay: Duration,
    },
}

impl RetryDecision {
    pub(super) const fn telemetry(self) -> RequestAttemptRetryDecision {
        match self {
            Self::Terminal => RequestAttemptRetryDecision::Terminal,
            Self::OAuthRefresh { .. } => RequestAttemptRetryDecision::OAuthRefresh,
            Self::RetrySamePath(_) => RequestAttemptRetryDecision::RetrySamePath,
            Self::Reselect { .. } => RequestAttemptRetryDecision::Reselect,
        }
    }

    pub(super) const fn delay(self) -> Option<Duration> {
        match self {
            Self::Terminal => None,
            Self::OAuthRefresh { delay, .. }
            | Self::RetrySamePath(delay)
            | Self::Reselect { delay, .. } => Some(delay),
        }
    }

    pub(super) const fn with_delay(self, delay: Duration) -> Self {
        match self {
            Self::Terminal => Self::Terminal,
            Self::OAuthRefresh {
                account_id,
                token_version,
                ..
            } => Self::OAuthRefresh {
                account_id,
                token_version,
                delay,
            },
            Self::RetrySamePath(_) => Self::RetrySamePath(delay),
            Self::Reselect { exclusion, .. } => Self::Reselect { exclusion, delay },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RetryExclusion {
    ExactCandidate,
    Credential,
    CredentialModel,
    RouteOperation,
    EgressPath,
    Proxy,
    Endpoint,
}

pub(super) fn retry_decision(
    failure: &AttemptFailure,
    budget: &RetryBudget,
    credential_id: RoutingCredentialId,
    can_refresh_oauth: bool,
) -> RetryDecision {
    if !failure.is_retry_candidate()
        || !failure.retry_safety().allows_automatic_retry()
        || !budget.can_retry()
    {
        return RetryDecision::Terminal;
    }
    let delay = budget.next_delay(credential_id, failure.retry_after());
    if can_refresh_oauth
        && let Some((account_id, token_version)) = failure.oauth_authentication_target()
    {
        return RetryDecision::OAuthRefresh {
            account_id,
            token_version,
            delay,
        };
    }
    if !failure.bound() {
        return RetryDecision::Reselect {
            exclusion: retry_exclusion(failure),
            delay,
        };
    }
    match failure {
        AttemptFailure::Upstream { error, .. }
            if matches!(
                error.classification().kind(),
                UpstreamErrorKind::Authentication
                    | UpstreamErrorKind::PermissionDenied
                    | UpstreamErrorKind::QuotaExhausted
                    | UpstreamErrorKind::ModelUnavailable
                    | UpstreamErrorKind::OperationUnavailable
            ) =>
        {
            RetryDecision::Terminal
        }
        AttemptFailure::Transport { .. }
        | AttemptFailure::Upstream { .. }
        | AttemptFailure::StreamRejected { .. } => RetryDecision::RetrySamePath(delay),
        AttemptFailure::Public(_) => RetryDecision::Terminal,
    }
}

fn retry_exclusion(failure: &AttemptFailure) -> RetryExclusion {
    match failure {
        AttemptFailure::Transport { error, .. } => match error.failure_scope {
            TransportFailureScope::Endpoint => RetryExclusion::Endpoint,
            TransportFailureScope::Proxy => RetryExclusion::Proxy,
            TransportFailureScope::EgressPath => RetryExclusion::EgressPath,
            TransportFailureScope::Unattributed => RetryExclusion::ExactCandidate,
        },
        AttemptFailure::Upstream { error, .. } => match error.classification().attribution() {
            UpstreamFailureAttribution::Authentication | UpstreamFailureAttribution::Credential => {
                RetryExclusion::Credential
            }
            UpstreamFailureAttribution::CredentialModel => RetryExclusion::CredentialModel,
            UpstreamFailureAttribution::RouteOperation => RetryExclusion::RouteOperation,
            UpstreamFailureAttribution::Endpoint => RetryExclusion::Endpoint,
            UpstreamFailureAttribution::EgressPath => RetryExclusion::EgressPath,
            UpstreamFailureAttribution::Unattributed => RetryExclusion::ExactCandidate,
        },
        AttemptFailure::StreamRejected { reason, .. } => match reason {
            StreamRetryReason::RateLimited => RetryExclusion::CredentialModel,
            StreamRetryReason::Overloaded => RetryExclusion::ExactCandidate,
        },
        AttemptFailure::Public(_) => RetryExclusion::ExactCandidate,
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
        AttemptFailure::StreamRejected { reason, .. } => match reason {
            StreamRetryReason::RateLimited => RequestAttemptFailureScope::CredentialModel,
            StreamRetryReason::Overloaded => RequestAttemptFailureScope::ExactCandidate,
        },
        AttemptFailure::Public(_) => return None,
    })
}

pub(super) fn exclude_failed_path(
    exclusions: &mut CandidateExclusions,
    failure: &AttemptFailure,
    scope: RetryExclusion,
) {
    let Some(candidate) = failure.candidate() else {
        return;
    };
    exclusions.note_failed_candidate(candidate);
    match scope {
        RetryExclusion::ExactCandidate => exclusions.exclude_candidate(candidate),
        RetryExclusion::Credential => exclusions.exclude_credential(candidate.credential_id),
        RetryExclusion::CredentialModel => exclusions.exclude_credential_model(candidate),
        RetryExclusion::RouteOperation => exclusions.exclude_target(candidate.target_id),
        RetryExclusion::EgressPath => {
            exclusions.exclude_egress_path(candidate.egress_path_identity());
        }
        RetryExclusion::Proxy => exclusions.exclude_proxy(candidate.proxy_id),
        RetryExclusion::Endpoint => exclusions.exclude_endpoint(candidate.endpoint_id),
    }
}
