use any2api_domain::HttpAccessLog;
use serde::Serialize;

use super::dto::SystemLogResponse;

#[derive(Serialize)]
pub(super) struct SystemLogDetailResponse {
    log: SystemLogResponse,
    has_request_log: bool,
}

impl SystemLogDetailResponse {
    pub(super) fn new(value: HttpAccessLog, has_request_log: bool) -> Self {
        let log = value.summary().into();
        Self {
            log,
            has_request_log,
        }
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ConfigRevision, HttpAccessLogOutcome, HttpProtocolVersion, RequestId};

    use super::*;

    #[test]
    fn detail_never_returns_raw_exchange_data() {
        let response = SystemLogDetailResponse::new(
            HttpAccessLog {
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
            },
            true,
        );

        let json = serde_json::to_value(response).expect("detail JSON");
        assert_eq!(json["log"]["path"], "/v1/responses");
        assert!(json["log"].get("exchange_captured").is_none());
        assert!(json.get("exchange").is_none());
    }
}
