use std::sync::Arc;

use any2api_domain::{PublicError, PublicErrorCode};
use tokio::sync::watch;

use crate::configuration::{GatewayApiKeyAuthProof, PublishedSnapshot, SnapshotStore};

/// Non-secret proof and revision feed used only while a request is still
/// Pending.  It allows route candidates to move to a newer snapshot without
/// carrying the plaintext Gateway API Key beyond ingress authentication.
pub(super) struct LiveRoutingSnapshots {
    snapshots: Arc<SnapshotStore>,
    changes: watch::Receiver<any2api_domain::ConfigRevision>,
    authentication: GatewayApiKeyAuthProof,
}

impl LiveRoutingSnapshots {
    pub(super) fn new(
        snapshots: Arc<SnapshotStore>,
        authentication: GatewayApiKeyAuthProof,
    ) -> Self {
        let changes = snapshots.subscribe_revision();
        Self {
            snapshots,
            changes,
            authentication,
        }
    }

    pub(super) async fn changed(&mut self) -> Result<(), PublicError> {
        self.changes.changed().await.map_err(|_| {
            PublicError::new(
                PublicErrorCode::InternalError,
                "configuration revision feed is unavailable",
            )
        })
    }

    pub(super) fn refresh(
        &mut self,
        current: &PublishedSnapshot,
    ) -> Option<Arc<PublishedSnapshot>> {
        // Mark the watch value first, then load ArcSwap. Snapshot publication
        // stores before notifying, so this ordering cannot lose a newer cutover.
        let _observed_revision = *self.changes.borrow_and_update();
        let latest = self.snapshots.load();
        if latest.revision() == current.revision() {
            return None;
        }
        Some(latest)
    }

    pub(super) fn validate(&self, candidate: &PublishedSnapshot) -> Result<(), PublicError> {
        let authenticated = candidate
            .gateway_api_keys()
            .get(self.authentication.id())
            .is_some_and(|key| {
                key.is_active() && key.token_version() == self.authentication.token_version()
            });
        if !authenticated {
            return Err(PublicError::new(
                PublicErrorCode::Unauthorized,
                "the Gateway API Key used for this request is no longer valid",
            ));
        }
        Ok(())
    }
}
