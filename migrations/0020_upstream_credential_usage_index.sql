CREATE INDEX request_logs_provider_credential_started_idx
    ON request_logs(credential_id, started_at_ms DESC, request_id DESC);
