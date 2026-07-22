use any2api_runtime::api::{ProcessLifecycle, RequestTelemetry, ShutdownPhase};
use any2api_storage::api::SqliteStore;

use super::ShutdownTimeouts;

pub(crate) async fn finalize(
    lifecycle: &ProcessLifecycle,
    telemetry: &RequestTelemetry,
    storage: &SqliteStore,
    timeouts: ShutdownTimeouts,
) -> anyhow::Result<()> {
    lifecycle.begin_draining();
    if lifecycle.active_requests() > 0 && lifecycle.phase() != ShutdownPhase::Forced {
        lifecycle.force();
    }
    tokio::time::timeout(timeouts.finalize, lifecycle.wait_for_requests())
        .await
        .map_err(|_| anyhow::anyhow!("HTTP requests did not stop before shutdown timeout"))?;

    lifecycle.close_background_tasks();
    telemetry.shutdown(timeouts.finalize).await;
    if tokio::time::timeout(timeouts.finalize, lifecycle.wait_for_background_tasks())
        .await
        .is_err()
    {
        lifecycle.force();
        tokio::time::timeout(timeouts.finalize, lifecycle.wait_for_background_tasks())
            .await
            .map_err(|_| {
                anyhow::anyhow!("background tasks did not stop before shutdown timeout")
            })?;
    }

    tokio::time::timeout(timeouts.finalize, storage.close())
        .await
        .map_err(|_| anyhow::anyhow!("sqlite pool did not close before shutdown timeout"))?;
    Ok(())
}
