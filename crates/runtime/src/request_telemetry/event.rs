use any2api_domain::{CompletedRequestLog, GatewayApiKeyId, HttpAccessLog};
use any2api_storage::api::StorageError;
use tokio::sync::oneshot;

use super::RequestTelemetryCheckpoint;
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
    QuotaCheckpoint {
        boundary: RequestTelemetryCheckpoint,
        reply: oneshot::Sender<RequestTelemetryCheckpoint>,
    },
}

pub(super) struct TelemetryEnvelope {
    pub(super) event: TelemetryEvent,
    pub(super) record_count: usize,
    pub(super) queue_class: TelemetryQueueClass,
    pub(super) owned_bytes: usize,
}

impl TelemetryEnvelope {
    pub(super) fn new(event: TelemetryEvent) -> Self {
        Self {
            record_count: event.record_count(),
            queue_class: event.queue_class(),
            owned_bytes: event.estimated_owned_bytes(),
            event,
        }
    }
}

impl TelemetryEvent {
    pub(super) const fn record_count(&self) -> usize {
        match self {
            Self::RequestLog(_) | Self::HttpAccessLog { .. } | Self::GatewayKeyLastUsed { .. } => 1,
            Self::ClearHttpAccessLogs { .. } | Self::QuotaCheckpoint { .. } => 0,
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

    fn estimated_owned_bytes(&self) -> usize {
        let envelope = std::mem::size_of::<TelemetryEnvelope>();
        match self {
            Self::RequestLog(record) => envelope.saturating_add(record.estimated_owned_bytes()),
            Self::HttpAccessLog { record, .. } => {
                envelope.saturating_add(record.estimated_owned_bytes())
            }
            Self::GatewayKeyLastUsed { last_used_at, .. } => {
                envelope.saturating_add(last_used_at.capacity())
            }
            Self::ClearHttpAccessLogs { .. } | Self::QuotaCheckpoint { .. } => 0,
        }
    }

    pub(super) const fn quota_relevant(&self) -> bool {
        matches!(self, Self::RequestLog(record) if record.request.oauth_account_id.is_some())
    }
}
