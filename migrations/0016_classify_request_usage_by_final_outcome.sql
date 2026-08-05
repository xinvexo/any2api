DROP INDEX request_logs_gateway_key_started_idx;
CREATE INDEX request_logs_gateway_key_started_idx
    ON request_logs(
        gateway_api_key_id,
        started_at_ms DESC,
        request_id DESC,
        status_code,
        error_class
    );

DROP INDEX request_logs_provider_credential_started_idx;
CREATE INDEX request_logs_provider_credential_started_idx
    ON request_logs(
        credential_id,
        started_at_ms DESC,
        request_id DESC,
        status_code,
        error_class
    );

DROP INDEX request_logs_oauth_account_idx;
CREATE INDEX request_logs_oauth_account_idx
    ON request_logs(
        oauth_account_id,
        started_at_ms DESC,
        status_code,
        error_class
    );
