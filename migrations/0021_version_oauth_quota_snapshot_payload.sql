CREATE TABLE oauth_quota_snapshots_v2 (
    oauth_account_id TEXT PRIMARY KEY NOT NULL
        REFERENCES oauth_accounts(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL CHECK (schema_version = 2),
    fetched_at INTEGER NOT NULL CHECK (fetched_at >= 0),
    payload BLOB NOT NULL CHECK (
        typeof(payload) = 'blob' AND length(payload) BETWEEN 2 AND 524288
    ),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

INSERT INTO oauth_quota_snapshots_v2 (
    oauth_account_id,
    schema_version,
    fetched_at,
    payload,
    updated_at
)
SELECT
    oauth_account_id,
    2,
    fetched_at,
    CAST(
        json_object(
            'usage',
            json(
                json_set(
                    CAST(payload AS TEXT),
                    '$.credits', NULL,
                    '$.access', NULL
                )
            ),
            'usd_estimates', json('[]')
        )
        AS BLOB
    ),
    updated_at
FROM oauth_quota_snapshots;

DROP TABLE oauth_quota_snapshots;
ALTER TABLE oauth_quota_snapshots_v2 RENAME TO oauth_quota_snapshots;
