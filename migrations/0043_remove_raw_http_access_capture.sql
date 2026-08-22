-- HTTP system logs are permanently metadata-only. Rebuild the table so raw
-- headers/bodies and query-bearing URI storage no longer exist in the current
-- schema, then reduce capacity accounting to the row limits still enforced.
DROP TRIGGER http_access_logs_capacity_stats_insert;
DROP TRIGGER http_access_logs_capacity_stats_delete;
DROP TRIGGER request_logs_capacity_stats_insert;
DROP TRIGGER request_logs_capacity_stats_delete;

CREATE TABLE http_access_logs_v43 (
    request_id TEXT PRIMARY KEY NOT NULL,
    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0),
    config_revision INTEGER NOT NULL CHECK (config_revision >= 1),
    client_ip TEXT CHECK (
        client_ip IS NULL
        OR (
            typeof(client_ip) = 'text'
            AND client_ip = trim(client_ip)
            AND length(client_ip) BETWEEN 2 AND 45
        )
    ),
    method TEXT NOT NULL CHECK (
        typeof(method) = 'text' AND length(method) >= 1 AND method = trim(method)
    ),
    path TEXT NOT NULL CHECK (typeof(path) = 'text' AND length(path) >= 1),
    http_version TEXT NOT NULL CHECK (
        http_version IN ('HTTP/0.9', 'HTTP/1.0', 'HTTP/1.1', 'HTTP/2', 'HTTP/3')
    ),
    status_code INTEGER CHECK (status_code IS NULL OR status_code BETWEEN 100 AND 999),
    duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0),
    response_bytes INTEGER NOT NULL CHECK (response_bytes >= 0),
    outcome TEXT NOT NULL CHECK (outcome IN ('completed', 'body_error', 'cancelled')),
    gateway_auth_rejected INTEGER NOT NULL DEFAULT 0
        CHECK (gateway_auth_rejected IN (0, 1))
) STRICT;

INSERT INTO http_access_logs_v43 (
    request_id,
    started_at_ms,
    config_revision,
    client_ip,
    method,
    path,
    http_version,
    status_code,
    duration_ms,
    response_bytes,
    outcome,
    gateway_auth_rejected
)
SELECT
    request_id,
    started_at_ms,
    config_revision,
    client_ip,
    method,
    path,
    http_version,
    status_code,
    duration_ms,
    response_bytes,
    outcome,
    gateway_auth_rejected
FROM http_access_logs;

DROP TABLE http_access_logs;
ALTER TABLE http_access_logs_v43 RENAME TO http_access_logs;

CREATE INDEX http_access_logs_started_idx
    ON http_access_logs(started_at_ms DESC, request_id DESC);
CREATE INDEX http_access_logs_summary_filter_idx
    ON http_access_logs(
        started_at_ms DESC,
        request_id DESC,
        path,
        client_ip,
        status_code,
        outcome
    );
CREATE INDEX http_access_logs_retention_idx
    ON http_access_logs(started_at_ms ASC, request_id ASC);
CREATE INDEX http_access_logs_gateway_auth_rejected_retention_idx
    ON http_access_logs(
        gateway_auth_rejected,
        started_at_ms ASC,
        request_id ASC
    );

CREATE TABLE telemetry_capacity_stats_v43 (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    request_log_rows INTEGER NOT NULL CHECK (request_log_rows >= 0),
    http_access_log_rows INTEGER NOT NULL CHECK (http_access_log_rows >= 0),
    gateway_auth_rejected_rows INTEGER NOT NULL
        CHECK (gateway_auth_rejected_rows >= 0)
) STRICT;

INSERT INTO telemetry_capacity_stats_v43 (
    singleton_id,
    request_log_rows,
    http_access_log_rows,
    gateway_auth_rejected_rows
)
SELECT
    1,
    (SELECT COUNT(*) FROM request_logs),
    (SELECT COUNT(*) FROM http_access_logs),
    (SELECT COALESCE(SUM(gateway_auth_rejected), 0) FROM http_access_logs);

DROP TABLE telemetry_capacity_stats;
ALTER TABLE telemetry_capacity_stats_v43 RENAME TO telemetry_capacity_stats;

CREATE TRIGGER http_access_logs_capacity_stats_insert
AFTER INSERT ON http_access_logs
BEGIN
    UPDATE telemetry_capacity_stats SET
        http_access_log_rows = http_access_log_rows + 1,
        gateway_auth_rejected_rows
            = gateway_auth_rejected_rows + NEW.gateway_auth_rejected
    WHERE singleton_id = 1;
END;

CREATE TRIGGER http_access_logs_capacity_stats_delete
AFTER DELETE ON http_access_logs
BEGIN
    UPDATE telemetry_capacity_stats SET
        http_access_log_rows = http_access_log_rows - 1,
        gateway_auth_rejected_rows
            = gateway_auth_rejected_rows - OLD.gateway_auth_rejected
    WHERE singleton_id = 1;
END;

CREATE TRIGGER request_logs_capacity_stats_insert
AFTER INSERT ON request_logs
BEGIN
    UPDATE telemetry_capacity_stats
    SET request_log_rows = request_log_rows + 1
    WHERE singleton_id = 1;
END;

CREATE TRIGGER request_logs_capacity_stats_delete
AFTER DELETE ON request_logs
BEGIN
    UPDATE telemetry_capacity_stats
    SET request_log_rows = request_log_rows - 1
    WHERE singleton_id = 1;
END;

DELETE FROM setting_overrides
WHERE key = 'logs.http_access.max_exchange_bytes';
