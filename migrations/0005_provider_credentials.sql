CREATE TABLE provider_credentials (
    id TEXT PRIMARY KEY,
    provider_endpoint_id TEXT NOT NULL
        REFERENCES provider_endpoints(id) ON DELETE RESTRICT,
    label TEXT NOT NULL CHECK (label = trim(label) AND length(label) BETWEEN 1 AND 100),
    label_key TEXT NOT NULL,
    credential_kind TEXT NOT NULL CHECK (credential_kind = 'api_key'),
    secret_schema_version INTEGER NOT NULL CHECK (secret_schema_version = 1),
    secret_version INTEGER NOT NULL CHECK (secret_version BETWEEN 1 AND 4294967295),
    credential_generation INTEGER NOT NULL CHECK (credential_generation BETWEEN 1 AND 4294967295),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    envelope_version INTEGER NOT NULL CHECK (envelope_version = 1),
    key_id TEXT NOT NULL CHECK (length(key_id) BETWEEN 1 AND 128),
    algorithm TEXT NOT NULL CHECK (algorithm = 'xchacha20poly1305'),
    nonce BLOB NOT NULL CHECK (length(nonce) = 24),
    ciphertext BLOB NOT NULL CHECK (length(ciphertext) >= 16),
    aad_version INTEGER NOT NULL CHECK (aad_version = 1),
    fingerprint_version INTEGER NOT NULL CHECK (fingerprint_version = 1),
    secret_fingerprint BLOB NOT NULL CHECK (length(secret_fingerprint) = 32),
    secret_tail TEXT CHECK (secret_tail IS NULL OR length(secret_tail) = 4),
    proxy_profile_id TEXT NOT NULL
        REFERENCES proxy_profiles(id) ON DELETE RESTRICT,
    max_concurrency INTEGER NOT NULL CHECK (max_concurrency BETWEEN 1 AND 10000),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(provider_endpoint_id, label_key)
);

CREATE INDEX provider_credentials_endpoint_idx
    ON provider_credentials(provider_endpoint_id, enabled);

CREATE INDEX provider_credentials_proxy_idx
    ON provider_credentials(proxy_profile_id);
