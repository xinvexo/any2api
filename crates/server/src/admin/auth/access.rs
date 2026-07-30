use std::{net::SocketAddr, sync::Arc};

use any2api_runtime::api::PublishedSnapshot;
use axum::http::HeaderMap;

use crate::{client_address, client_address::ClientConnection};

use super::error::AdminApiError;

pub(super) fn resolve(
    snapshot: Arc<PublishedSnapshot>,
    peer: Option<SocketAddr>,
    headers: &HeaderMap,
) -> Result<(ClientConnection, Arc<PublishedSnapshot>), AdminApiError> {
    let connection = client_address::resolve(
        snapshot.settings().network().trusted_proxy_cidrs(),
        peer,
        headers,
    )
    .map_err(|_| AdminApiError::invalid_forwarded_headers())?;
    if !connection.is_loopback() && !snapshot.settings().admin().remote_enabled() {
        return Err(AdminApiError::remote_disabled());
    }
    Ok((connection, snapshot))
}
