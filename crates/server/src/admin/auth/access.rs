use std::{net::SocketAddr, sync::Arc};

use any2api_runtime::api::PublishedSnapshot;
use axum::http::HeaderMap;

use crate::{client_address::ClientConnection, state::AppState};

use super::error::AdminApiError;

pub(super) fn resolve(
    state: &AppState,
    peer: Option<SocketAddr>,
    headers: &HeaderMap,
) -> Result<(ClientConnection, Arc<PublishedSnapshot>), AdminApiError> {
    let connection = state
        .client_addresses()
        .resolve(peer, headers)
        .map_err(|_| AdminApiError::invalid_forwarded_headers())?;
    let snapshot = state.snapshots().load();
    if !connection.is_loopback() && !snapshot.settings().admin().remote_enabled() {
        return Err(AdminApiError::remote_disabled());
    }
    Ok((connection, snapshot))
}
