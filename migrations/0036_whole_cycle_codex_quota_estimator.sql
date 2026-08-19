-- Estimator v10 learns from complete official quota cycles. The cumulative
-- v9 interval totals cannot be reconstructed into a whole-cycle baseline, so
-- preserve the last official usage snapshot and cold-start estimator state.
CREATE TABLE oauth_quota_snapshots_v10 (
    oauth_account_id TEXT PRIMARY KEY NOT NULL
        REFERENCES oauth_accounts(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL CHECK (schema_version = 10),
    fetched_at INTEGER NOT NULL CHECK (fetched_at >= 0),
    payload BLOB NOT NULL CHECK (
        typeof(payload) = 'blob' AND length(payload) BETWEEN 2 AND 524288
    ),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

INSERT INTO oauth_quota_snapshots_v10 (
    oauth_account_id,
    schema_version,
    fetched_at,
    payload,
    updated_at
)
SELECT
    oauth_account_id,
    10,
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
ALTER TABLE oauth_quota_snapshots_v10 RENAME TO oauth_quota_snapshots;

CREATE INDEX request_logs_oauth_quota_completion_idx
    ON request_logs(oauth_account_id, (started_at_ms + latency_ms))
    WHERE oauth_account_id IS NOT NULL;
