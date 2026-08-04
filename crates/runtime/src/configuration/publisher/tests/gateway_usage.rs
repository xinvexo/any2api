use std::sync::Arc;

use any2api_domain::{ConfigRevision, GatewayApiKeyDraft, GatewayApiKeyId};

use super::TestContext;
use crate::{
    configuration::{ConfigPublisher, PublishedSnapshotReconciler},
    request_telemetry::RequestTelemetry,
};

#[tokio::test]
async fn deleting_a_gateway_key_prunes_live_usage_and_blocks_old_snapshot_reinsertion() {
    let context = TestContext::new().await;
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&context.repository),
        ConfigRevision::INITIAL,
        context.snapshots.load().settings().logging(),
        &context.runtime.lifecycle(),
    ));
    let publisher = ConfigPublisher::new(
        Arc::clone(&context.repository),
        Arc::clone(&context.snapshots),
        Arc::clone(&context.runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher")
    .with_snapshot_reconciler(Arc::clone(&telemetry) as Arc<dyn PublishedSnapshotReconciler>);
    let id = GatewayApiKeyId::new();

    let created = publisher
        .create_gateway_api_key(
            ConfigRevision::INITIAL,
            id,
            GatewayApiKeyDraft::new("Churned Key", true).expect("Gateway Key draft"),
        )
        .await
        .expect("create Gateway Key");
    telemetry.record_gateway_key_use(id, created.revision());
    assert!(telemetry.gateway_key_last_used_at(id).is_some());
    let config_version = created
        .gateway_api_keys()
        .get(id)
        .expect("created Gateway Key")
        .config_version();

    let deleted = publisher
        .delete_gateway_api_key(created.revision(), id, config_version)
        .await
        .expect("delete Gateway Key");

    assert!(deleted.gateway_api_keys().get(id).is_none());
    assert!(telemetry.gateway_key_last_used_at(id).is_none());
    telemetry.record_gateway_key_use(id, created.revision());
    assert!(telemetry.gateway_key_last_used_at(id).is_none());
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}
