use any2api_domain::{HttpAccessLog, HttpAccessLogOutcome};

pub(super) fn should_record(log: &HttpAccessLog) -> bool {
    is_public_proxy_path(&log.path)
        || log.client_ip.is_none_or(|address| !address.is_loopback())
        || log.status_code.is_none_or(|status| status >= 400)
        || log.outcome != HttpAccessLogOutcome::Completed
}

fn is_public_proxy_path(path: &str) -> bool {
    path == "/v1" || path.starts_with("/v1/")
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ConfigRevision, HttpProtocolVersion, RequestId};

    use super::*;

    #[test]
    fn keeps_proxy_external_and_error_traffic_but_drops_normal_local_access() {
        let mut log = fixture("/api/admin/request-logs", "127.0.0.1", 200);
        assert!(!should_record(&log));

        log.path = "/v1/responses".to_owned();
        assert!(should_record(&log));

        log.path = "/V1/responses".to_owned();
        assert!(!should_record(&log));

        log.path = "/api/health".to_owned();
        log.client_ip = Some("203.0.113.8".parse().expect("external address"));
        assert!(should_record(&log));

        log.client_ip = Some("::1".parse().expect("loopback address"));
        log.status_code = Some(500);
        assert!(should_record(&log));

        log.status_code = Some(200);
        log.outcome = HttpAccessLogOutcome::BodyError;
        assert!(should_record(&log));
    }

    fn fixture(path: &str, client_ip: &str, status_code: u16) -> HttpAccessLog {
        HttpAccessLog {
            request_id: RequestId::new(),
            started_at_ms: 1,
            config_revision: ConfigRevision::INITIAL,
            client_ip: Some(client_ip.parse().expect("client address")),
            method: "GET".to_owned(),
            path: path.to_owned(),
            http_version: HttpProtocolVersion::Http11,
            status_code: Some(status_code),
            duration_ms: 1,
            response_bytes: 1,
            outcome: HttpAccessLogOutcome::Completed,
        }
    }
}
