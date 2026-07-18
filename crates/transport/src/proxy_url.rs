use any2api_domain::{ProxyKind, ProxyProfile};
use url::Url;

use crate::{TransportError, TransportErrorStage};

pub(crate) fn proxy_url(profile: &ProxyProfile) -> Result<Option<Url>, TransportError> {
    if profile.is_built_in() {
        return Ok(None);
    }
    let address = profile
        .address()
        .ok_or_else(|| TransportError::configuration("configured proxy has no network address"))?;
    let scheme = match profile.kind() {
        ProxyKind::Direct => return Ok(None),
        ProxyKind::Http => "http",
        ProxyKind::Socks5 => "socks5h",
    };
    let mut url = Url::parse(&format!("{scheme}://localhost"))
        .map_err(|_| TransportError::configuration("failed to construct configured proxy URL"))?;
    url.set_host(Some(address.host()))
        .map_err(|_| TransportError::configuration("configured proxy host is invalid"))?;
    url.set_port(Some(address.port()))
        .map_err(|_| TransportError::configuration("configured proxy port is invalid"))?;
    Ok(Some(url))
}

impl TransportError {
    pub(crate) fn configuration(message: impl Into<String>) -> Self {
        Self::new(
            TransportErrorStage::ProxyHandshake,
            any2api_domain::RetrySafety::DefinitelyNotSent,
            message,
        )
    }
}
