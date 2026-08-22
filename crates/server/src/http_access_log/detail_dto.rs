use any2api_domain::HttpAccessLog;
use serde::Serialize;

use super::dto::SystemLogResponse;

#[derive(Serialize)]
pub(super) struct SystemLogDetailResponse {
    log: SystemLogResponse,
}

impl From<HttpAccessLog> for SystemLogDetailResponse {
    fn from(value: HttpAccessLog) -> Self {
        let log = value.summary().into();
        Self { log }
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ConfigRevision, HttpAccessLogOutcome, HttpProtocolVersion, RequestId};

    use super::*;

    #[test]
    fn detail_never_returns_raw_exchange_data() {
        let response = SystemLogDetailResponse::from(HttpAccessLog {
            request_id: RequestId::new(),
            started_at_ms: 1,
            config_revision: ConfigRevision::INITIAL,
            client_ip: None,
            method: "POST".to_owned(),
            path: "/v1/responses".to_owned(),
            http_version: HttpProtocolVersion::Http11,
            status_code: Some(200),
            duration_ms: 1,
            response_bytes: 2,
            outcome: HttpAccessLogOutcome::Completed,
            gateway_auth_rejected: false,
        });

        let json = serde_json::to_value(response).expect("detail JSON");
        assert_eq!(json["log"]["path"], "/v1/responses");
        assert!(json["log"].get("exchange_captured").is_none());
        assert!(json.get("exchange").is_none());
    }
}
