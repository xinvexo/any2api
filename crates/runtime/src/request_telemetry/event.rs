use any2api_domain::{CompletedRequestLog, GatewayApiKeyId, HttpAccessLog};
use any2api_storage::api::StorageError;
use tokio::sync::oneshot;

pub(super) enum TelemetryEvent {
    RequestLog(Box<CompletedRequestLog>),
    HttpAccessLog(Box<HttpAccessLog>),
    GatewayKeyLastUsed {
        id: GatewayApiKeyId,
        last_used_at: String,
    },
    ClearHttpAccessLogs {
        reply: oneshot::Sender<Result<u64, StorageError>>,
    },
}
