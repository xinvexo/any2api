-- The old token format is intentionally not read or rewritten. Operators must
-- explicitly remove old GatewayApiKey rows before adopting the current format.
SELECT CASE
    WHEN EXISTS (SELECT 1 FROM gateway_api_keys)
    THEN abs(-9223372036854775808)
    ELSE 0
END;

CREATE TABLE gateway_api_keys_v4 (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (name = trim(name) AND length(name) BETWEEN 1 AND 100),
    name_key TEXT NOT NULL UNIQUE,
    token TEXT NOT NULL CHECK (
        length(token) = 46
        AND substr(token, 1, 3) = 'sk-'
        AND token NOT GLOB '*[^A-Za-z0-9_-]*'
    ),
    token_prefix TEXT NOT NULL CHECK (length(token_prefix) BETWEEN 1 AND 64),
    token_hash BLOB NOT NULL UNIQUE CHECK (length(token_hash) = 32),
    hash_version INTEGER NOT NULL CHECK (hash_version = 2),
    token_version INTEGER NOT NULL CHECK (token_version BETWEEN 1 AND 4294967295),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

DROP TABLE gateway_api_keys;
ALTER TABLE gateway_api_keys_v4 RENAME TO gateway_api_keys;
CREATE INDEX gateway_api_keys_enabled_name_idx
    ON gateway_api_keys(enabled, name);
