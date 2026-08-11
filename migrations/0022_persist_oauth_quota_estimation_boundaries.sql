CREATE TABLE oauth_quota_estimation_boundaries (
    oauth_account_id TEXT PRIMARY KEY NOT NULL
        REFERENCES oauth_accounts(id) ON DELETE CASCADE,
    reset_at_ms INTEGER NOT NULL CHECK (reset_at_ms >= 0),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;
