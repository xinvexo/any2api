CREATE TABLE oauth_quota_snapshots_v7 (
    oauth_account_id TEXT PRIMARY KEY NOT NULL
        REFERENCES oauth_accounts(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL CHECK (schema_version = 7),
    fetched_at INTEGER NOT NULL CHECK (fetched_at >= 0),
    payload BLOB NOT NULL CHECK (
        typeof(payload) = 'blob' AND length(payload) BETWEEN 2 AND 524288
    ),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

INSERT INTO oauth_quota_snapshots_v7 (
    oauth_account_id,
    schema_version,
    fetched_at,
    payload,
    updated_at
)
SELECT
    oauth_account_id,
    7,
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
ALTER TABLE oauth_quota_snapshots_v7 RENAME TO oauth_quota_snapshots;
