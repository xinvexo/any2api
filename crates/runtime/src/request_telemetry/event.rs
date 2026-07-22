use any2api_domain::{CompletedRequestLog, GatewayApiKeyId};

pub(super) enum TelemetryEvent {
    RequestLog(Box<CompletedRequestLog>),
    GatewayKeyLastUsed {
        id: GatewayApiKeyId,
        last_used_at: String,
    },
}
