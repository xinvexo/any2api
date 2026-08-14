CREATE TABLE oauth_accounts_v31 (
    id TEXT PRIMARY KEY,
    provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'claude', 'grok')),
    label TEXT NOT NULL CHECK (label = trim(label) AND length(label) BETWEEN 1 AND 100),
    label_key TEXT NOT NULL,
    oauth_json BLOB NOT NULL CHECK (
        typeof(oauth_json) = 'blob' AND length(oauth_json) BETWEEN 2 AND 65536
    ),
    token_version INTEGER NOT NULL CHECK (token_version BETWEEN 1 AND 4294967295),
    account_generation INTEGER NOT NULL
        CHECK (account_generation BETWEEN 1 AND 4294967295),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    proxy_profile_id TEXT REFERENCES proxy_profiles(id) ON DELETE RESTRICT,
    requests_per_minute INTEGER CHECK (
        requests_per_minute IS NULL OR requests_per_minute BETWEEN 1 AND 100000
    ),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    safe_account_email TEXT CHECK (
        safe_account_email IS NULL
        OR (
            safe_account_email = trim(safe_account_email)
            AND length(safe_account_email) BETWEEN 1 AND 320
        )
    ),
    expires_at INTEGER CHECK (expires_at IS NULL OR expires_at >= 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(provider_kind, label_key)
);

INSERT INTO oauth_accounts_v31 (
    id,
    provider_kind,
    label,
    label_key,
    oauth_json,
    token_version,
    account_generation,
    config_version,
    proxy_profile_id,
    requests_per_minute,
    enabled,
    safe_account_email,
    expires_at,
    created_at,
    updated_at
)
SELECT
    id,
    provider_kind,
    label,
    label_key,
    oauth_json,
    token_version,
    account_generation,
    config_version,
    NULL,
    requests_per_minute,
    enabled,
    safe_account_email,
    expires_at,
    created_at,
    updated_at
FROM oauth_accounts;

DROP TABLE oauth_accounts;
ALTER TABLE oauth_accounts_v31 RENAME TO oauth_accounts;

CREATE INDEX oauth_accounts_provider_enabled_idx
    ON oauth_accounts(provider_kind, enabled);
CREATE INDEX oauth_accounts_expiry_idx
    ON oauth_accounts(enabled, expires_at);
CREATE INDEX oauth_accounts_proxy_idx
    ON oauth_accounts(proxy_profile_id);
