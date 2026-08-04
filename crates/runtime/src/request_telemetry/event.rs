use any2api_domain::{CompletedRequestLog, GatewayApiKeyId, HttpAccessLog};
use any2api_storage::api::StorageError;
use tokio::sync::oneshot;

use super::metrics::TelemetryQueueClass;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpAccessLogChangeNotification {
    Notify,
    Suppress,
}

impl HttpAccessLogChangeNotification {
    pub(super) const fn should_notify(self) -> bool {
        matches!(self, Self::Notify)
    }
}

pub(super) enum TelemetryEvent {
    RequestLog(Box<CompletedRequestLog>),
    HttpAccessLog {
        record: Box<HttpAccessLog>,
        notification: HttpAccessLogChangeNotification,
    },
    GatewayKeyLastUsed {
        id: GatewayApiKeyId,
        last_used_at: String,
    },
    ClearHttpAccessLogs {
        reply: oneshot::Sender<Result<u64, StorageError>>,
    },
}

impl TelemetryEvent {
    pub(super) const fn record_count(&self) -> usize {
        match self {
            Self::RequestLog(_) | Self::HttpAccessLog { .. } | Self::GatewayKeyLastUsed { .. } => 1,
            Self::ClearHttpAccessLogs { .. } => 0,
        }
    }

    pub(super) fn queue_class(&self) -> TelemetryQueueClass {
        match self {
            Self::HttpAccessLog { record, .. } if record.gateway_auth_rejected => {
                TelemetryQueueClass::GatewayAuthRejected
            }
            _ => TelemetryQueueClass::Regular,
        }
    }
}
