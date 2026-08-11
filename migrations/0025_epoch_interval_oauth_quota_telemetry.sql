ALTER TABLE request_logs ADD COLUMN quota_cost_unit TEXT;
ALTER TABLE request_logs ADD COLUMN quota_cost_nanos INTEGER;
ALTER TABLE request_logs ADD COLUMN quota_cost_rate_card TEXT;
ALTER TABLE request_logs ADD COLUMN quota_service_tier TEXT CHECK (
    (
        quota_cost_unit IS NULL
        AND quota_cost_nanos IS NULL
        AND quota_cost_rate_card IS NULL
        AND quota_service_tier IS NULL
    )
    OR (
        quota_cost_unit = 'codex_credits'
        AND quota_cost_nanos >= 0
        AND typeof(quota_cost_nanos) = 'integer'
        AND length(quota_cost_rate_card) BETWEEN 1 AND 128
        AND quota_service_tier IN ('standard', 'fast')
    )
);

CREATE INDEX request_logs_oauth_quota_completion_idx
    ON request_logs(oauth_account_id, (started_at_ms + latency_ms))
    WHERE oauth_account_id IS NOT NULL;

CREATE TABLE oauth_quota_snapshots_v5 (
    oauth_account_id TEXT PRIMARY KEY NOT NULL
        REFERENCES oauth_accounts(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL CHECK (schema_version = 5),
    fetched_at INTEGER NOT NULL CHECK (fetched_at >= 0),
    payload BLOB NOT NULL CHECK (
        typeof(payload) = 'blob' AND length(payload) BETWEEN 2 AND 524288
    ),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

INSERT INTO oauth_quota_snapshots_v5 (
    oauth_account_id,
    schema_version,
    fetched_at,
    payload,
    updated_at
)
SELECT
    oauth_account_id,
    5,
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
ALTER TABLE oauth_quota_snapshots_v5 RENAME TO oauth_quota_snapshots;
DROP TABLE oauth_quota_estimation_boundaries;
