ALTER TABLE http_access_logs
    ADD COLUMN gateway_auth_rejected INTEGER NOT NULL DEFAULT 0
        CHECK (gateway_auth_rejected IN (0, 1));

CREATE INDEX http_access_logs_gateway_auth_rejected_retention_idx
    ON http_access_logs(
        gateway_auth_rejected,
        started_at_ms ASC,
        request_id ASC,
        exchange_bytes
    );
