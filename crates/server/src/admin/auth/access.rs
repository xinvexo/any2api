use std::sync::Arc;

use any2api_runtime::api::PublishedSnapshot;

use crate::client_address::{ClientAddressContext, ClientConnection};

use super::error::AdminApiError;

pub(super) fn resolve(
    context: ClientAddressContext,
) -> Result<(ClientConnection, Arc<PublishedSnapshot>), AdminApiError> {
    let connection = context
        .connection()
        .map_err(|_| AdminApiError::invalid_forwarded_headers())?;
    let snapshot = context.snapshot_arc();
    if !connection.is_direct_loopback() && !snapshot.settings().admin().remote_enabled() {
        return Err(AdminApiError::remote_disabled());
    }
    Ok((connection, snapshot))
}
