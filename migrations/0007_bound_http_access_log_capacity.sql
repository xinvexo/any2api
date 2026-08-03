ALTER TABLE http_access_logs
    ADD COLUMN exchange_bytes INTEGER NOT NULL DEFAULT 0
        CHECK (exchange_bytes >= 0);

UPDATE http_access_logs
SET exchange_bytes = CASE exchange_captured
    WHEN 1 THEN length(request_headers) + length(request_body)
        + length(response_headers) + length(response_body)
    ELSE 0
END;

CREATE INDEX http_access_logs_retention_idx
    ON http_access_logs(started_at_ms ASC, request_id ASC, exchange_bytes);
