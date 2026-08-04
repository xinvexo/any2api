use std::sync::Arc;

use any2api_domain::RetrySafety;
use http::Uri;

use crate::error::{TransportError, TransportErrorStage, TransportFailureScope};

/// Stable identity of an upstream origin parsed from the request URI without
/// any DNS resolution, so cached clients survive DNS rotation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct OriginTarget {
    pub(crate) host: Arc<str>,
    pub(crate) port: u16,
    pub(crate) secure: bool,
}

pub(crate) fn origin_target(uri: &Uri) -> Result<OriginTarget, TransportError> {
    let host = uri.host().ok_or_else(|| {
        TransportError::new(
            TransportErrorStage::Dns,
            TransportFailureScope::Endpoint,
            RetrySafety::DefinitelyNotSent,
            "upstream URI has no host",
        )
    })?;
    let port = uri
        .port_u16()
        .or_else(|| match uri.scheme_str() {
            Some("http") => Some(80),
            Some("https") => Some(443),
            _ => None,
        })
        .ok_or_else(|| {
            TransportError::new(
                TransportErrorStage::Dns,
                TransportFailureScope::Endpoint,
                RetrySafety::DefinitelyNotSent,
                "upstream URI has no port",
            )
        })?;
    Ok(OriginTarget {
        host: Arc::from(host.to_owned()),
        port,
        secure: uri.scheme_str() == Some("https"),
    })
}
