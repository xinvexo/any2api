use std::{net::SocketAddr, sync::Arc};

use any2api_runtime::api::PublishedSnapshot;
use axum::http::HeaderMap;

use crate::{admin_auth::AdminConnection, state::AppState};

use super::error::AdminApiError;

pub(super) fn resolve(
    state: &AppState,
    peer: Option<SocketAddr>,
    headers: &HeaderMap,
) -> Result<(AdminConnection, Arc<PublishedSnapshot>), AdminApiError> {
    let connection = state
        .admin_network()
        .resolve(peer, headers)
        .map_err(|_| AdminApiError::invalid_forwarded_headers())?;
    let snapshot = state.snapshots().load();
    if !connection.is_loopback() && !snapshot.settings().admin().remote_enabled() {
        return Err(AdminApiError::remote_disabled());
    }
    Ok((connection, snapshot))
}
