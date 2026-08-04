CREATE TABLE telemetry_capacity_stats (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    request_log_rows INTEGER NOT NULL CHECK (request_log_rows >= 0),
    http_access_log_rows INTEGER NOT NULL CHECK (http_access_log_rows >= 0),
    http_access_log_exchange_bytes INTEGER NOT NULL
        CHECK (http_access_log_exchange_bytes >= 0),
    gateway_auth_rejected_rows INTEGER NOT NULL
        CHECK (gateway_auth_rejected_rows >= 0),
    gateway_auth_rejected_exchange_bytes INTEGER NOT NULL
        CHECK (gateway_auth_rejected_exchange_bytes >= 0)
) STRICT;

INSERT INTO telemetry_capacity_stats (
    singleton_id,
    request_log_rows,
    http_access_log_rows,
    http_access_log_exchange_bytes,
    gateway_auth_rejected_rows,
    gateway_auth_rejected_exchange_bytes
)
SELECT
    1,
    (SELECT COUNT(*) FROM request_logs),
    (SELECT COUNT(*) FROM http_access_logs),
    (SELECT COALESCE(SUM(exchange_bytes), 0) FROM http_access_logs),
    (SELECT COALESCE(SUM(gateway_auth_rejected), 0) FROM http_access_logs),
    (SELECT COALESCE(SUM(CASE gateway_auth_rejected
         WHEN 1 THEN exchange_bytes ELSE 0 END), 0) FROM http_access_logs);

CREATE TRIGGER http_access_logs_capacity_stats_insert
AFTER INSERT ON http_access_logs
BEGIN
    UPDATE telemetry_capacity_stats SET
        http_access_log_rows = http_access_log_rows + 1,
        http_access_log_exchange_bytes
            = http_access_log_exchange_bytes + NEW.exchange_bytes,
        gateway_auth_rejected_rows
            = gateway_auth_rejected_rows + NEW.gateway_auth_rejected,
        gateway_auth_rejected_exchange_bytes
            = gateway_auth_rejected_exchange_bytes
                + CASE NEW.gateway_auth_rejected
                    WHEN 1 THEN NEW.exchange_bytes ELSE 0 END
    WHERE singleton_id = 1;
END;

CREATE TRIGGER http_access_logs_capacity_stats_delete
AFTER DELETE ON http_access_logs
BEGIN
    UPDATE telemetry_capacity_stats SET
        http_access_log_rows = http_access_log_rows - 1,
        http_access_log_exchange_bytes
            = http_access_log_exchange_bytes - OLD.exchange_bytes,
        gateway_auth_rejected_rows
            = gateway_auth_rejected_rows - OLD.gateway_auth_rejected,
        gateway_auth_rejected_exchange_bytes
            = gateway_auth_rejected_exchange_bytes
                - CASE OLD.gateway_auth_rejected
                    WHEN 1 THEN OLD.exchange_bytes ELSE 0 END
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
