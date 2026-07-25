use std::error::Error as _;

use any2api_domain::{ProxyKind, ProxyProfile, RetrySafety};
use http::Uri;

use crate::error::{TransportError, TransportErrorStage, TransportFailureScope};

pub(super) fn map_send_error(
    profile: &ProxyProfile,
    uses_http_forward_proxy: bool,
    error: reqwest::Error,
) -> TransportError {
    if error.is_connect() {
        if profile.kind() == ProxyKind::Http && is_proxy_authentication_error(&error) {
            TransportError::proxy_unavailable("configured proxy authentication was rejected")
        } else if profile.kind() == ProxyKind::Direct {
            TransportError::new(
                TransportErrorStage::Tcp,
                TransportFailureScope::Endpoint,
                RetrySafety::DefinitelyNotSent,
                "direct connection failed",
            )
        } else if uses_http_forward_proxy {
            TransportError::proxy_unavailable("configured proxy connection failed")
        } else {
            TransportError::new(
                TransportErrorStage::ProxyHandshake,
                TransportFailureScope::Unattributed,
                RetrySafety::DefinitelyNotSent,
                "proxied connection failed",
            )
        }
    } else if error.is_timeout() {
        TransportError::new(
            TransportErrorStage::AwaitHeaders,
            failure_scope_for_unverified_path(profile),
            RetrySafety::Ambiguous,
            "upstream response headers timed out",
        )
    } else {
        TransportError::new(
            TransportErrorStage::AwaitHeaders,
            failure_scope_for_unverified_path(profile),
            RetrySafety::Ambiguous,
            "upstream request failed before response headers",
        )
    }
}

// reqwest 0.12.28 does not re-export hyper-util's TunnelError, so the typed
// CONNECT 407 case is identified through its stable source display and pinned by a real test.
fn is_proxy_authentication_error(error: &reqwest::Error) -> bool {
    let mut source = error.source();
    while let Some(cause) = source {
        if cause.to_string() == "tunnel error: proxy authorization required" {
            return true;
        }
        source = cause.source();
    }
    false
}

pub(super) fn validate_uri(uri: &Uri) -> Result<(), TransportError> {
    match uri.scheme_str() {
        Some("http" | "https") => Ok(()),
        _ => Err(TransportError::new(
            TransportErrorStage::WriteRequest,
            TransportFailureScope::Unattributed,
            RetrySafety::DefinitelyNotSent,
            "transport only supports HTTP and HTTPS upstream URIs",
        )),
    }
}

pub(super) fn failure_scope_for_unverified_path(profile: &ProxyProfile) -> TransportFailureScope {
    if profile.kind() == ProxyKind::Direct {
        TransportFailureScope::Endpoint
    } else {
        TransportFailureScope::Unattributed
    }
}
