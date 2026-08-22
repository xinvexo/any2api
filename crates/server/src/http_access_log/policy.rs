use std::net::IpAddr;

use any2api_domain::{HttpAccessLog, HttpAccessLogOutcome, is_loopback_ip};
use any2api_runtime::api::HttpAccessLogChangeNotification;

pub(super) fn should_record(
    path: &str,
    client_ip: Option<IpAddr>,
    status_code: Option<u16>,
    outcome: HttpAccessLogOutcome,
) -> bool {
    is_public_proxy_path(path)
        || client_ip.is_none_or(|address| !is_loopback_ip(address))
        || status_code.is_none_or(|status| status >= 400)
        || outcome != HttpAccessLogOutcome::Completed
}

pub(super) fn change_notification(log: &HttpAccessLog) -> HttpAccessLogChangeNotification {
    if log.method == "GET" && log.path == "/api/admin/system-logs" {
        HttpAccessLogChangeNotification::Suppress
    } else {
        HttpAccessLogChangeNotification::Notify
    }
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
        let loopback = Some("127.0.0.1".parse().expect("loopback"));
        assert!(!should_record(
            "/api/admin/request-logs",
            loopback,
            Some(200),
            HttpAccessLogOutcome::Completed,
        ));
        assert!(should_record(
            "/v1/responses",
            loopback,
            Some(200),
            HttpAccessLogOutcome::Completed,
        ));
        assert!(!should_record(
            "/V1/responses",
            loopback,
            Some(200),
            HttpAccessLogOutcome::Completed,
        ));
        assert!(!should_record(
            "/api/health",
            Some("::ffff:127.0.0.1".parse().expect("mapped loopback")),
            Some(200),
            HttpAccessLogOutcome::Completed,
        ));
        assert!(should_record(
            "/api/health",
            Some("203.0.113.8".parse().expect("external address")),
            Some(200),
            HttpAccessLogOutcome::Completed,
        ));
        assert!(should_record(
            "/api/health",
            Some("::1".parse().expect("loopback address")),
            Some(500),
            HttpAccessLogOutcome::Completed,
        ));
        assert!(should_record(
            "/api/health",
            loopback,
            Some(200),
            HttpAccessLogOutcome::BodyError,
        ));
    }

    #[test]
    fn system_log_reads_are_audited_without_notifying_their_own_watchers() {
        let mut log = fixture("/api/admin/system-logs", "203.0.113.8", 200);
        assert!(should_record(
            &log.path,
            log.client_ip,
            log.status_code,
            log.outcome,
        ));
        assert_eq!(
            change_notification(&log),
            HttpAccessLogChangeNotification::Suppress
        );

        log.method = "DELETE".to_owned();
        assert_eq!(
            change_notification(&log),
            HttpAccessLogChangeNotification::Notify
        );
        log.method = "GET".to_owned();
        log.path = "/api/admin/system-logs/archive".to_owned();
        assert_eq!(
            change_notification(&log),
            HttpAccessLogChangeNotification::Notify
        );
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
            gateway_auth_rejected: false,
        }
    }
}
