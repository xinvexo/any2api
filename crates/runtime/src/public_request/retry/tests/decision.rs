use std::time::Duration;

use any2api_domain::{
    RequestAttemptFailureScope, RetryAfterHint, RetrySafety, UpstreamErrorKind,
    UpstreamFailureAttribution,
};
use any2api_protocol::api::ProtocolRetryDelayBasis;
use any2api_transport::api::{TransportError, TransportErrorStage, TransportFailureScope};

use super::support::{
    attempted_budget, candidate, upstream_failure, upstream_failure_with_retry_after,
};
use crate::{
    public_request::{
        retry::decision::{RetryDecision, retry_decision, telemetry_failure_scope},
        upstream::{AttemptFailure, UpstreamFailureOrigin},
    },
    routing::CandidateFailureScope,
};

#[test]
fn hard_upstream_failures_reselect_their_attributed_scope() {
    let candidate = candidate("bad-key");
    let cases = [
        (
            UpstreamFailureAttribution::Unattributed,
            CandidateFailureScope::ExactCandidate,
        ),
        (
            UpstreamFailureAttribution::Authentication,
            CandidateFailureScope::Credential,
        ),
        (
            UpstreamFailureAttribution::Credential,
            CandidateFailureScope::Credential,
        ),
        (
            UpstreamFailureAttribution::CredentialModel,
            CandidateFailureScope::CredentialModel,
        ),
        (
            UpstreamFailureAttribution::RouteOperation,
            CandidateFailureScope::RouteOperation,
        ),
        (
            UpstreamFailureAttribution::EgressPath,
            CandidateFailureScope::EgressPath,
        ),
        (
            UpstreamFailureAttribution::Endpoint,
            CandidateFailureScope::Endpoint,
        ),
    ];

    for (attribution, scope) in cases {
        let failure = upstream_failure(
            candidate.clone(),
            false,
            UpstreamErrorKind::Authentication,
            RetrySafety::RejectedBeforeExecution,
            attribution,
        );
        let budget = attempted_budget(&candidate);
        assert_eq!(
            retry_decision(&failure, &budget, false),
            RetryDecision::Reselect { scope },
            "attribution {attribution:?}",
        );
    }
}

#[test]
fn safe_transport_failures_reselect_without_waiting() {
    let candidate = candidate("transport");
    for (failure_scope, scope) in [
        (
            TransportFailureScope::Endpoint,
            CandidateFailureScope::Endpoint,
        ),
        (TransportFailureScope::Proxy, CandidateFailureScope::Proxy),
        (
            TransportFailureScope::EgressPath,
            CandidateFailureScope::EgressPath,
        ),
        (
            TransportFailureScope::Unattributed,
            CandidateFailureScope::ExactCandidate,
        ),
    ] {
        let failure = AttemptFailure::Transport {
            error: Box::new(TransportError::new(
                TransportErrorStage::Tcp,
                failure_scope,
                RetrySafety::DefinitelyNotSent,
                "test failure",
            )),
            candidate: Box::new(candidate.clone()),
            bound: false,
        };
        let budget = attempted_budget(&candidate);
        assert_eq!(
            retry_decision(&failure, &budget, false),
            RetryDecision::Reselect { scope },
        );
    }
}

#[test]
fn bound_requests_never_switch_and_only_retry_safe_transient_failures_repeat() {
    let candidate = candidate("bound");
    let hard = upstream_failure(
        candidate.clone(),
        true,
        UpstreamErrorKind::Authentication,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::Authentication,
    );
    let budget = attempted_budget(&candidate);
    assert_eq!(
        retry_decision(&hard, &budget, false),
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
        retry_decision(&transient, &budget, false),
        RetryDecision::RetrySamePath(_)
    ));

    let ambiguous = upstream_failure(
        candidate.clone(),
        true,
        UpstreamErrorKind::Transient,
        RetrySafety::Ambiguous,
        UpstreamFailureAttribution::Unattributed,
    );
    assert_eq!(
        retry_decision(&ambiguous, &budget, false),
        RetryDecision::Terminal
    );
}

#[test]
fn retry_after_defers_only_the_failed_scope_for_unbound_requests() {
    let candidate = candidate("retry-after");
    let budget = attempted_budget(&candidate);
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
        retry_decision(&unbound, &budget, false),
        RetryDecision::PreferAlternate {
            scope: CandidateFailureScope::CredentialModel,
            retry_delay: Duration::from_secs(7),
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
        retry_decision(&bound, &budget, false),
        RetryDecision::RetrySamePath(Duration::from_secs(7))
    );
}

#[test]
fn complete_ambiguous_upstream_failure_prefers_an_alternate_candidate() {
    let candidate = candidate("ambiguous-complete");
    let failure = upstream_failure(
        candidate.clone(),
        false,
        UpstreamErrorKind::Transient,
        RetrySafety::Ambiguous,
        UpstreamFailureAttribution::Unattributed,
    );
    let budget = attempted_budget(&candidate);
    assert_eq!(
        retry_decision(&failure, &budget, false),
        RetryDecision::PreferAlternate {
            scope: CandidateFailureScope::ExactCandidate,
            retry_delay: Duration::from_secs(1),
        }
    );
}

#[test]
fn incomplete_http_error_body_is_terminal() {
    let candidate = candidate("incomplete-body");
    let mut failure = upstream_failure(
        candidate.clone(),
        false,
        UpstreamErrorKind::Transient,
        RetrySafety::Ambiguous,
        UpstreamFailureAttribution::Unattributed,
    );
    let AttemptFailure::Upstream { origin, .. } = &mut failure else {
        unreachable!()
    };
    *origin = UpstreamFailureOrigin::HttpStatus {
        error_body_complete: false,
    };
    let budget = attempted_budget(&candidate);
    assert_eq!(
        retry_decision(&failure, &budget, false),
        RetryDecision::Terminal
    );
}

#[test]
fn oauth_authentication_gets_one_refresh_decision_before_hard_reselection() {
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
    let budget = attempted_budget(&candidate);

    assert_eq!(
        retry_decision(&failure, &budget, true),
        RetryDecision::OAuthRefresh {
            account_id,
            token_version: candidate.binding.generation().authentication_version(),
        }
    );
    assert_eq!(
        retry_decision(&failure, &budget, false),
        RetryDecision::Reselect {
            scope: CandidateFailureScope::Credential,
        }
    );
}

#[test]
fn presemantic_stream_overload_uses_request_attempt_backoff() {
    let first = candidate("first-overload");
    let second = candidate("second-overload");
    let mut budget = attempted_budget(&first);
    assert_eq!(budget.register_attempt(&second), Some(2));
    let mut failure = upstream_failure(
        second.clone(),
        false,
        UpstreamErrorKind::Transient,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::Unattributed,
    );
    let AttemptFailure::Upstream { origin, .. } = &mut failure else {
        unreachable!()
    };
    *origin = UpstreamFailureOrigin::PreSemanticStreamEnvelope {
        retry_delay_basis: ProtocolRetryDelayBasis::RequestAttempts,
    };

    assert_eq!(
        retry_decision(&failure, &budget, false),
        RetryDecision::PreferAlternate {
            scope: CandidateFailureScope::ExactCandidate,
            retry_delay: Duration::from_secs(2),
        }
    );
    assert_eq!(
        telemetry_failure_scope(&failure),
        Some(RequestAttemptFailureScope::ExactCandidate)
    );
}

#[test]
fn invalid_request_is_never_retried() {
    let candidate = candidate("invalid");
    let failure = upstream_failure(
        candidate.clone(),
        false,
        UpstreamErrorKind::InvalidRequest,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::RouteOperation,
    );
    let budget = attempted_budget(&candidate);
    assert_eq!(
        retry_decision(&failure, &budget, false),
        RetryDecision::Terminal
    );
}
