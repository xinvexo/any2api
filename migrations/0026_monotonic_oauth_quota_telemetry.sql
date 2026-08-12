ALTER TABLE request_logs ADD COLUMN telemetry_process_id TEXT;
ALTER TABLE request_logs ADD COLUMN telemetry_sequence INTEGER CHECK (
    (
        telemetry_process_id IS NULL
        AND telemetry_sequence IS NULL
    )
    OR (
        telemetry_process_id IS NOT NULL
        AND length(telemetry_process_id) = 36
        AND telemetry_sequence >= 1
        AND typeof(telemetry_sequence) = 'integer'
    )
);

DROP INDEX request_logs_oauth_quota_completion_idx;

CREATE UNIQUE INDEX request_logs_telemetry_position_idx
    ON request_logs(telemetry_process_id, telemetry_sequence)
    WHERE telemetry_process_id IS NOT NULL;

CREATE INDEX request_logs_oauth_quota_sequence_idx
    ON request_logs(oauth_account_id, telemetry_process_id, telemetry_sequence)
    WHERE oauth_account_id IS NOT NULL AND telemetry_process_id IS NOT NULL;

CREATE TABLE oauth_quota_snapshots_v6 (
    oauth_account_id TEXT PRIMARY KEY NOT NULL
        REFERENCES oauth_accounts(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL CHECK (schema_version = 6),
    fetched_at INTEGER NOT NULL CHECK (fetched_at >= 0),
    payload BLOB NOT NULL CHECK (
        typeof(payload) = 'blob' AND length(payload) BETWEEN 2 AND 524288
    ),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

INSERT INTO oauth_quota_snapshots_v6 (
    oauth_account_id,
    schema_version,
    fetched_at,
    payload,
    updated_at
)
SELECT
    oauth_account_id,
    6,
    fetched_at,
    CAST(
        json_object(
            'usage', json(json_extract(CAST(payload AS TEXT), '$.usage')),
            'estimator_state', NULL
        )
        AS BLOB
    ),
    updated_at
FROM oauth_quota_snapshots;

DROP TABLE oauth_quota_snapshots;
ALTER TABLE oauth_quota_snapshots_v6 RENAME TO oauth_quota_snapshots;
