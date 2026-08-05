use any2api_domain::RetrySafety;
use any2api_transport::api::{TransportError, TransportErrorStage, TransportFailureScope};

use super::failure::{
    OAuthRefreshError, OAuthRefreshFailureReason, OAuthRefreshFailureScope,
    OAuthRefreshFailureStage, OAuthRefreshTrigger,
};
use crate::oauth::error::OAuthError;

#[test]
fn transport_failures_preserve_stage_and_scope_without_exposing_the_message() {
    let error = OAuthRefreshError::OAuth(OAuthError::Transport(TransportError::new(
        TransportErrorStage::ProxyHandshake,
        TransportFailureScope::Proxy,
        RetrySafety::DefinitelyNotSent,
        "proxy-secret.example:8443",
    )));
    let failure = error.failure(7, OAuthRefreshTrigger::Scheduled);

    assert_eq!(failure.token_version(), 7);
    assert_eq!(failure.stage(), OAuthRefreshFailureStage::ProxyHandshake);
    assert_eq!(
        failure.reason(),
        OAuthRefreshFailureReason::TransportFailure
    );
    assert_eq!(
        failure.failure_scope(),
        Some(OAuthRefreshFailureScope::Proxy)
    );
    assert!(!error.to_string().contains("proxy-secret"));
}

#[test]
fn response_read_timeout_is_distinct_from_transport_and_endpoint_rejection() {
    let failure = OAuthRefreshError::OAuth(OAuthError::TokenReadTimeout)
        .failure(3, OAuthRefreshTrigger::AuthenticationFailure);

    assert_eq!(failure.stage(), OAuthRefreshFailureStage::ReadResponse);
    assert_eq!(failure.reason(), OAuthRefreshFailureReason::ReadTimeout);
    assert_eq!(failure.upstream_status(), None);
    assert!(!failure.reauthorization_required());
}
