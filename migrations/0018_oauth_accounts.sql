CREATE TABLE oauth_accounts (
    id TEXT PRIMARY KEY,
    provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'claude')),
    label TEXT NOT NULL CHECK (label = trim(label) AND length(label) BETWEEN 1 AND 100),
    label_key TEXT NOT NULL,
    oauth_json BLOB NOT NULL CHECK (
        typeof(oauth_json) = 'blob'
        AND length(oauth_json) BETWEEN 2 AND 65536
    ),
    token_version INTEGER NOT NULL CHECK (token_version BETWEEN 1 AND 4294967295),
    account_generation INTEGER NOT NULL CHECK (account_generation BETWEEN 1 AND 4294967295),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    proxy_profile_id TEXT NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000'
        REFERENCES proxy_profiles(id) ON DELETE RESTRICT
        CHECK (proxy_profile_id = '00000000-0000-0000-0000-000000000000'),
    max_concurrency INTEGER NOT NULL CHECK (max_concurrency BETWEEN 1 AND 10000),
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

CREATE TABLE oauth_account_models (
    oauth_account_id TEXT NOT NULL
        REFERENCES oauth_accounts(id) ON DELETE CASCADE,
    upstream_model TEXT NOT NULL CHECK (
        upstream_model = trim(upstream_model)
        AND length(upstream_model) BETWEEN 1 AND 255
    ),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (oauth_account_id, upstream_model)
);

CREATE INDEX oauth_accounts_provider_enabled_idx
    ON oauth_accounts(provider_kind, enabled);
CREATE INDEX oauth_accounts_expiry_idx
    ON oauth_accounts(enabled, expires_at);
CREATE INDEX oauth_account_models_model_idx
    ON oauth_account_models(upstream_model, oauth_account_id);
