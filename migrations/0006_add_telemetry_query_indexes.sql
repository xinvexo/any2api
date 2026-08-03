CREATE INDEX request_attempts_route_target_idx
    ON request_attempts(route_target_id);

CREATE INDEX request_attempts_credential_idx
    ON request_attempts(credential_id);

CREATE INDEX request_attempts_proxy_profile_idx
    ON request_attempts(proxy_profile_id);

CREATE INDEX request_logs_provider_endpoint_idx
    ON request_logs(provider_endpoint_id);

CREATE INDEX request_logs_proxy_profile_idx
    ON request_logs(proxy_profile_id);

CREATE INDEX http_access_logs_summary_filter_idx
    ON http_access_logs(
        started_at_ms DESC,
        request_id DESC,
        path,
        client_ip,
        status_code,
        outcome
    );
