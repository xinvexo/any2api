CREATE INDEX request_logs_gateway_key_started_idx
    ON request_logs(gateway_api_key_id, started_at_ms DESC, request_id DESC);
