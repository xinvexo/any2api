use std::sync::Arc;

use any2api_domain::{GatewayApiKeyDraft, GatewayApiKeyId};
use any2api_runtime::api::{
    ConfigPublisher, GatewayApiKeyAuthProof, PublishedSnapshot, SnapshotStore,
};

pub struct TestGatewayAuthentication {
    pub snapshot: Arc<PublishedSnapshot>,
    pub proof: GatewayApiKeyAuthProof,
}

pub async fn create_gateway_authentication(
    publisher: &ConfigPublisher,
    snapshots: &SnapshotStore,
) -> TestGatewayAuthentication {
    let current = snapshots.load();
    let id = GatewayApiKeyId::new();
    let snapshot = publisher
        .create_gateway_api_key(
            current.revision(),
            id,
            GatewayApiKeyDraft::new("contract request", true).expect("Gateway key draft"),
        )
        .await
        .expect("Gateway key");
    let token = snapshot
        .gateway_api_keys()
        .get(id)
        .expect("created Gateway key")
        .token();
    let proof = snapshot
        .authenticate_gateway_api_key(token)
        .expect("created Gateway key authenticates");
    TestGatewayAuthentication { snapshot, proof }
}
