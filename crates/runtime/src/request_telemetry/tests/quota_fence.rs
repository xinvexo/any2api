use std::sync::{Arc, atomic::Ordering};

use any2api_domain::{ConfigRevision, OAuthAccountId, RequestId};

use super::support::{BlockingRepository, logging_settings, oauth_record, wait_for};
use crate::{lifecycle::ProcessLifecycle, request_telemetry::RequestTelemetry};

#[tokio::test]
async fn quota_observation_waits_for_queued_oauth_logs() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    ));
    let policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());
    let account = OAuthAccountId::new();

    telemetry.try_record(oauth_record(RequestId::new(), account), policy);
    wait_for(|| repository.write_batches.load(Ordering::Acquire) == 1).await;
    let task = {
        let telemetry = Arc::clone(&telemetry);
        tokio::spawn(async move { telemetry.quota_observation().await })
    };
    tokio::task::yield_now().await;
    assert!(!task.is_finished());
    repository.release_first.notify_waiters();
    let observation = task.await.expect("quota observation");
    assert_eq!(observation.position.sequence, 1);
    assert_eq!(
        repository.request_logs.lock().unwrap()[0]
            .telemetry_position
            .expect("telemetry position"),
        observation.position
    );

    telemetry.try_record(oauth_record(RequestId::new(), account), policy);
    let next = telemetry.quota_observation().await;
    assert_eq!(next.position.sequence, 2);
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}
