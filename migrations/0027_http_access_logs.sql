CREATE TABLE http_access_logs (
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
        typeof(method) = 'text'
        AND length(method) BETWEEN 1 AND 32
        AND method = trim(method)
    ),
    path TEXT NOT NULL CHECK (
        typeof(path) = 'text'
        AND length(path) >= 1
    ),
    http_version TEXT NOT NULL CHECK (
        http_version IN ('HTTP/0.9', 'HTTP/1.0', 'HTTP/1.1', 'HTTP/2', 'HTTP/3')
    ),
    status_code INTEGER CHECK (status_code IS NULL OR status_code BETWEEN 100 AND 999),
    duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0),
    response_bytes INTEGER NOT NULL CHECK (response_bytes >= 0),
    outcome TEXT NOT NULL CHECK (outcome IN ('completed', 'body_error', 'cancelled'))
) STRICT;

CREATE INDEX http_access_logs_started_idx
ON http_access_logs (started_at_ms DESC, request_id DESC);
