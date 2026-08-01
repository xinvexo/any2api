use std::sync::Arc;

use any2api_domain::{ConfigRevision, GatewayApiKeyId};

use super::support::{BlockingRepository, logging_settings, wait_for};
use crate::{lifecycle::ProcessLifecycle, request_telemetry::RequestTelemetry};

#[tokio::test]
async fn gateway_key_usage_is_live_immediately_and_duplicate_writes_are_throttled() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let id = GatewayApiKeyId::new();

    telemetry.record_gateway_key_use(id);
    let first = telemetry
        .gateway_key_last_used_at(id)
        .expect("live usage timestamp");
    telemetry.record_gateway_key_use(id);
    wait_for(|| {
        repository
            .usage_updates
            .lock()
            .expect("usage updates")
            .len()
            == 1
    })
    .await;

    let latest = telemetry
        .gateway_key_last_used_at(id)
        .expect("latest live usage timestamp");
    assert!(latest >= first);
    assert_eq!(
        repository
            .usage_updates
            .lock()
            .expect("usage updates")
            .len(),
        1
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}
