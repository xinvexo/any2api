use any2api_domain::{ErrorClass, RequestAttempt, RequestAttemptOutcome, RequestId, RetrySafety};

use super::final_error_class;

#[test]
fn final_error_prefers_the_last_attempt_class() {
    assert_eq!(
        final_error_class(
            &[attempt(
                ErrorClass::RateLimited,
                RequestAttemptOutcome::UpstreamError
            )],
            Some(ErrorClass::Upstream),
        ),
        Some(ErrorClass::RateLimited)
    );
}

#[test]
fn explicit_error_overrides_an_internal_timeout_cancellation() {
    assert_eq!(
        final_error_class(
            &[attempt(
                ErrorClass::Cancelled,
                RequestAttemptOutcome::Cancelled
            )],
            Some(ErrorClass::Upstream),
        ),
        Some(ErrorClass::Upstream)
    );
}

fn attempt(error_class: ErrorClass, outcome: RequestAttemptOutcome) -> RequestAttempt {
    RequestAttempt {
        request_id: RequestId::new(),
        attempt_no: 1,
        route_target_id: None,
        credential_id: None,
        oauth_account_id: None,
        proxy_profile_id: None,
        started_at_ms: 0,
        duration_ms: 0,
        retry_safety: Some(RetrySafety::Ambiguous),
        error_class: Some(error_class),
        status_code: None,
        outcome,
    }
}
