-- SQLite evaluates only the selected CASE branch; abs(INT64_MIN) is a read-only
-- assertion that raises integer overflow when any rejected row exists.
SELECT CASE
    WHEN EXISTS (SELECT 1 FROM gateway_api_keys)
      OR EXISTS (SELECT 1 FROM provider_credentials)
      OR EXISTS (SELECT 1 FROM proxy_passwords)
    THEN abs(-9223372036854775808)
    ELSE 0
END;

CREATE TABLE gateway_api_keys_v3 (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (name = trim(name) AND length(name) BETWEEN 1 AND 100),
    name_key TEXT NOT NULL UNIQUE,
    token TEXT NOT NULL CHECK (
        length(token) = 50
        AND substr(token, 1, 7) = 'a2k_v1_'
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
ALTER TABLE gateway_api_keys_v3 RENAME TO gateway_api_keys;
CREATE INDEX gateway_api_keys_enabled_name_idx
    ON gateway_api_keys(enabled, name);

CREATE TABLE proxy_passwords_v3 (
    proxy_profile_id TEXT PRIMARY KEY
        REFERENCES proxy_profiles(id) ON DELETE CASCADE,
    username TEXT NOT NULL CHECK (length(username) BETWEEN 1 AND 255),
    authentication_version INTEGER NOT NULL
        CHECK (authentication_version BETWEEN 1 AND 4294967295),
    password BLOB NOT NULL CHECK (
        typeof(password) = 'blob' AND length(password) BETWEEN 1 AND 255
    ),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (proxy_profile_id <> '00000000-0000-0000-0000-000000000000')
);

DROP TABLE proxy_passwords;
ALTER TABLE proxy_passwords_v3 RENAME TO proxy_passwords;

CREATE TRIGGER proxy_password_version_must_match
BEFORE INSERT ON proxy_passwords
WHEN NOT EXISTS (
    SELECT 1 FROM proxy_profiles
    WHERE id = NEW.proxy_profile_id
      AND kind IN ('http', 'socks5')
      AND authentication_version = NEW.authentication_version
)
BEGIN
    SELECT RAISE(ABORT, 'proxy_password_version_mismatch');
END;

CREATE TRIGGER proxy_password_update_version_must_match
BEFORE UPDATE ON proxy_passwords
WHEN NOT EXISTS (
    SELECT 1 FROM proxy_profiles
    WHERE id = NEW.proxy_profile_id
      AND kind IN ('http', 'socks5')
      AND authentication_version = NEW.authentication_version
)
BEGIN
    SELECT RAISE(ABORT, 'proxy_password_version_mismatch');
END;

CREATE TABLE provider_credentials_v3 (
    id TEXT PRIMARY KEY,
    provider_endpoint_id TEXT NOT NULL
        REFERENCES provider_endpoints(id) ON DELETE RESTRICT,
    label TEXT NOT NULL CHECK (label = trim(label) AND length(label) BETWEEN 1 AND 100),
    label_key TEXT NOT NULL,
    credential_kind TEXT NOT NULL CHECK (credential_kind = 'api_key'),
    secret_version INTEGER NOT NULL CHECK (secret_version BETWEEN 1 AND 4294967295),
    credential_generation INTEGER NOT NULL
        CHECK (credential_generation BETWEEN 1 AND 4294967295),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    api_key BLOB NOT NULL CHECK (
        typeof(api_key) = 'blob' AND length(api_key) BETWEEN 1 AND 8192
    ),
    fingerprint_version INTEGER NOT NULL CHECK (fingerprint_version = 2),
    secret_fingerprint BLOB NOT NULL CHECK (length(secret_fingerprint) = 32),
    secret_tail TEXT CHECK (secret_tail IS NULL OR length(secret_tail) = 4),
    proxy_profile_id TEXT NOT NULL
        REFERENCES proxy_profiles(id) ON DELETE RESTRICT,
    requests_per_minute INTEGER CHECK (
        requests_per_minute IS NULL OR requests_per_minute BETWEEN 1 AND 100000
    ),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(provider_endpoint_id, label_key)
);

DROP TABLE provider_credentials;
ALTER TABLE provider_credentials_v3 RENAME TO provider_credentials;
CREATE INDEX provider_credentials_endpoint_idx
    ON provider_credentials(provider_endpoint_id, enabled);
CREATE INDEX provider_credentials_proxy_idx
    ON provider_credentials(proxy_profile_id);

DROP TABLE secret_vault_metadata;
