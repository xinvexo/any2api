use std::time::Duration;

use any2api_domain::{
    RequestAttemptFailureScope, RetryAfterHint, RetrySafety, UpstreamErrorKind,
    UpstreamFailureAttribution,
};
use any2api_protocol::api::{StreamRejection, StreamRetryReason};
use any2api_transport::api::{TransportError, TransportErrorStage, TransportFailureScope};

use super::support::{
    attempted_budget, candidate, upstream_failure, upstream_failure_with_retry_after,
};
use crate::public_request::{
    retry::decision::{RetryDecision, RetryExclusion, retry_decision, telemetry_failure_scope},
    upstream::AttemptFailure,
};

#[test]
fn unbound_retry_safe_upstream_failures_choose_their_exact_scope() {
    let candidate = candidate("bad-key");
    let cases = [
        (
            UpstreamFailureAttribution::Unattributed,
            RetryExclusion::ExactCandidate,
        ),
        (
            UpstreamFailureAttribution::Authentication,
            RetryExclusion::Credential,
        ),
        (
            UpstreamFailureAttribution::Credential,
            RetryExclusion::Credential,
        ),
        (
            UpstreamFailureAttribution::CredentialModel,
            RetryExclusion::CredentialModel,
        ),
        (
            UpstreamFailureAttribution::RouteOperation,
            RetryExclusion::RouteOperation,
        ),
        (
            UpstreamFailureAttribution::EgressPath,
            RetryExclusion::EgressPath,
        ),
        (
            UpstreamFailureAttribution::Endpoint,
            RetryExclusion::Endpoint,
        ),
    ];

    for (attribution, expected) in cases {
        let failure = upstream_failure(
            candidate.clone(),
            false,
            UpstreamErrorKind::Authentication,
            RetrySafety::RejectedBeforeExecution,
            attribution,
        );
        let budget = attempted_budget(candidate.credential_id);
        assert_eq!(
            retry_decision(&failure, &budget, candidate.credential_id, false),
            RetryDecision::Reselect {
                exclusion: expected,
                delay: Duration::from_secs(1),
            },
            "attribution {attribution:?}",
        );
    }
}

#[test]
fn transport_scope_drives_reselection_without_guessing_the_endpoint() {
    let candidate = candidate("transport");
    for (scope, expected) in [
        (TransportFailureScope::Endpoint, RetryExclusion::Endpoint),
        (TransportFailureScope::Proxy, RetryExclusion::Proxy),
        (
            TransportFailureScope::EgressPath,
            RetryExclusion::EgressPath,
        ),
        (
            TransportFailureScope::Unattributed,
            RetryExclusion::ExactCandidate,
        ),
    ] {
        let failure = AttemptFailure::Transport {
            error: Box::new(TransportError::new(
                TransportErrorStage::Tcp,
                scope,
                RetrySafety::DefinitelyNotSent,
                "test failure",
            )),
            candidate: Box::new(candidate.clone()),
            bound: false,
        };
        let budget = attempted_budget(candidate.credential_id);
        assert_eq!(
            retry_decision(&failure, &budget, candidate.credential_id, false),
            RetryDecision::Reselect {
                exclusion: expected,
                delay: Duration::from_secs(1),
            },
            "scope {scope:?}",
        );
    }
}

#[test]
fn bound_requests_never_switch_credentials_or_targets() {
    let candidate = candidate("bound");
    let deterministic = upstream_failure(
        candidate.clone(),
        true,
        UpstreamErrorKind::Authentication,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::Authentication,
    );
    let budget = attempted_budget(candidate.credential_id);
    assert_eq!(
        retry_decision(&deterministic, &budget, candidate.credential_id, false),
        RetryDecision::Terminal
    );

    let transient = upstream_failure(
        candidate.clone(),
        true,
        UpstreamErrorKind::RateLimited,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::CredentialModel,
    );
    assert!(matches!(
        retry_decision(&transient, &budget, candidate.credential_id, false),
        RetryDecision::RetrySamePath(_)
    ));

    let stream_rate_limit = stream_rejection(
        candidate.clone(),
        true,
        StreamRetryReason::RateLimited,
        "rate_limit_error",
    );
    assert!(matches!(
        retry_decision(&stream_rate_limit, &budget, candidate.credential_id, false),
        RetryDecision::RetrySamePath(_)
    ));
}

#[test]
fn retry_after_is_the_same_minimum_for_reselect_and_same_path_retry() {
    let candidate = candidate("retry-after");
    let budget = attempted_budget(candidate.credential_id);
    let retry_after = Some(RetryAfterHint::Delay(Duration::from_secs(7)));
    let unbound = upstream_failure_with_retry_after(
        candidate.clone(),
        false,
        UpstreamErrorKind::RateLimited,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::CredentialModel,
        retry_after,
    );
    assert_eq!(
        retry_decision(&unbound, &budget, candidate.credential_id, false),
        RetryDecision::Reselect {
            exclusion: RetryExclusion::CredentialModel,
            delay: Duration::from_secs(7),
        }
    );

    let bound = upstream_failure_with_retry_after(
        candidate.clone(),
        true,
        UpstreamErrorKind::RateLimited,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::CredentialModel,
        retry_after,
    );
    assert_eq!(
        retry_decision(&bound, &budget, candidate.credential_id, false),
        RetryDecision::RetrySamePath(Duration::from_secs(7))
    );
}

#[test]
fn ambiguous_server_failure_is_terminal_even_for_an_unbound_request() {
    let candidate = candidate("ambiguous");
    let failure = upstream_failure(
        candidate.clone(),
        false,
        UpstreamErrorKind::Transient,
        RetrySafety::Ambiguous,
        UpstreamFailureAttribution::Unattributed,
    );
    let budget = attempted_budget(candidate.credential_id);

    assert_eq!(
        retry_decision(&failure, &budget, candidate.credential_id, false),
        RetryDecision::Terminal
    );
}

#[test]
fn cache_locality_forgets_path_failures_but_keeps_client_rejections() {
    let candidate = candidate("cache-locality");
    for kind in [
        UpstreamErrorKind::Authentication,
        UpstreamErrorKind::PermissionDenied,
        UpstreamErrorKind::QuotaExhausted,
        UpstreamErrorKind::RateLimited,
        UpstreamErrorKind::ModelUnavailable,
        UpstreamErrorKind::OperationUnavailable,
        UpstreamErrorKind::Transient,
    ] {
        let failure = upstream_failure(
            candidate.clone(),
            false,
            kind,
            kind.default_retry_safety(),
            UpstreamFailureAttribution::Unattributed,
        );
        assert!(
            failure.cache_locality_failure_candidate().is_some(),
            "{kind:?} must invalidate the failed cache-locality path"
        );
    }

    for kind in [
        UpstreamErrorKind::InvalidRequest,
        UpstreamErrorKind::Unknown,
    ] {
        let failure = upstream_failure(
            candidate.clone(),
            false,
            kind,
            kind.default_retry_safety(),
            UpstreamFailureAttribution::Unattributed,
        );
        assert!(
            failure.cache_locality_failure_candidate().is_none(),
            "{kind:?} must preserve a previously successful path"
        );
    }
}

#[test]
fn oauth_authentication_gets_one_refresh_decision_before_reselection() {
    let mut candidate = candidate("oauth");
    let account_id = any2api_domain::OAuthAccountId::new();
    candidate.credential_id = any2api_domain::RoutingCredentialId::oauth_account(account_id);
    let failure = upstream_failure(
        candidate.clone(),
        false,
        UpstreamErrorKind::Authentication,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::Authentication,
    );
    let budget = attempted_budget(candidate.credential_id);

    assert_eq!(
        retry_decision(&failure, &budget, candidate.credential_id, true),
        RetryDecision::OAuthRefresh {
            account_id,
            token_version: candidate.binding.generation().authentication_version(),
            delay: Duration::from_secs(1),
        }
    );
    assert_eq!(
        retry_decision(&failure, &budget, candidate.credential_id, false),
        RetryDecision::Reselect {
            exclusion: RetryExclusion::Credential,
            delay: Duration::from_secs(1),
        }
    );
}

#[test]
fn precontent_rate_limit_stream_reselects_by_credential_model() {
    let candidate = candidate("rate-limited-stream");
    let failure = stream_rejection(
        candidate.clone(),
        false,
        StreamRetryReason::RateLimited,
        "rate_limit_error",
    );
    let budget = attempted_budget(candidate.credential_id);

    assert_eq!(
        retry_decision(&failure, &budget, candidate.credential_id, false),
        RetryDecision::Reselect {
            exclusion: RetryExclusion::CredentialModel,
            delay: Duration::from_secs(1),
        }
    );
    assert_eq!(
        telemetry_failure_scope(&failure),
        Some(RequestAttemptFailureScope::CredentialModel)
    );
}

#[test]
fn precontent_overload_backoff_keeps_growing_after_a_credential_switch() {
    let first = candidate("first-overloaded");
    let second = candidate("second-overloaded");
    let mut budget = attempted_budget(first.credential_id);
    assert_eq!(budget.register_attempt(second.credential_id), Some(2));
    let failure = stream_rejection(
        second.clone(),
        false,
        StreamRetryReason::Overloaded,
        "server_is_overloaded",
    );

    assert_eq!(
        retry_decision(&failure, &budget, second.credential_id, false),
        RetryDecision::Reselect {
            exclusion: RetryExclusion::ExactCandidate,
            delay: Duration::from_secs(2),
        }
    );
    assert_eq!(
        telemetry_failure_scope(&failure),
        Some(RequestAttemptFailureScope::ExactCandidate)
    );
}

fn stream_rejection(
    candidate: crate::routing::RouteCandidate,
    bound: bool,
    reason: StreamRetryReason,
    code: &'static str,
) -> AttemptFailure {
    AttemptFailure::StreamRejected {
        status: http::StatusCode::OK,
        headers: Box::new(http::HeaderMap::new()),
        body: bytes::Bytes::from_static(b"event: error\ndata: {}\n\n"),
        candidate: Box::new(candidate),
        bound,
        rejection: StreamRejection::new(reason, code),
    }
}
