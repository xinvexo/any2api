CREATE TABLE gateway_api_keys (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (name = trim(name) AND length(name) BETWEEN 1 AND 100),
    name_key TEXT NOT NULL UNIQUE,
    token_prefix TEXT NOT NULL CHECK (length(token_prefix) BETWEEN 1 AND 64),
    token_hash BLOB NOT NULL UNIQUE CHECK (length(token_hash) = 32),
    hash_version INTEGER NOT NULL CHECK (hash_version = 1),
    hash_key_id TEXT NOT NULL CHECK (length(hash_key_id) BETWEEN 1 AND 128),
    token_version INTEGER NOT NULL CHECK (token_version BETWEEN 1 AND 4294967295),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    revoked_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (revoked_at IS NULL OR enabled = 0)
);

CREATE INDEX gateway_api_keys_active_idx
    ON gateway_api_keys(enabled, revoked_at, name);
