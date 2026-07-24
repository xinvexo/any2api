PRAGMA defer_foreign_keys = ON;

CREATE TEMP TABLE migration_0017_credential_kind_guard (
    unsupported_oauth_credentials INTEGER NOT NULL CHECK (unsupported_oauth_credentials = 0)
);
INSERT INTO migration_0017_credential_kind_guard (unsupported_oauth_credentials)
SELECT COUNT(*)
FROM provider_credentials
WHERE credential_kind <> 'api_key';
DROP TABLE migration_0017_credential_kind_guard;

CREATE TABLE provider_credentials_api_key (
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
    secret_tail TEXT CHECK (
        secret_tail IS NULL
        OR (credential_kind = 'api_key' AND length(secret_tail) = 4)
    ),
    proxy_profile_id TEXT NOT NULL
        REFERENCES proxy_profiles(id) ON DELETE RESTRICT,
    max_concurrency INTEGER NOT NULL CHECK (max_concurrency BETWEEN 1 AND 10000),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(provider_endpoint_id, label_key)
);

INSERT INTO provider_credentials_api_key (
    id, provider_endpoint_id, label, label_key, credential_kind, secret_schema_version,
    secret_version, credential_generation, config_version, envelope_version, key_id,
    algorithm, nonce, ciphertext, aad_version, fingerprint_version, secret_fingerprint,
    secret_tail, proxy_profile_id, max_concurrency, enabled, created_at, updated_at
)
SELECT
    id, provider_endpoint_id, label, label_key, credential_kind, secret_schema_version,
    secret_version, credential_generation, config_version, envelope_version, key_id,
    algorithm, nonce, ciphertext, aad_version, fingerprint_version, secret_fingerprint,
    secret_tail, proxy_profile_id, max_concurrency, enabled, created_at, updated_at
FROM provider_credentials;

CREATE TABLE provider_credential_models_api_key (
    credential_id TEXT NOT NULL
        REFERENCES provider_credentials_api_key(id) ON DELETE CASCADE,
    upstream_model TEXT NOT NULL CHECK (
        upstream_model = trim(upstream_model)
        AND length(upstream_model) BETWEEN 1 AND 255
    ),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (credential_id, upstream_model)
);

INSERT INTO provider_credential_models_api_key (credential_id, upstream_model, created_at)
SELECT credential_id, upstream_model, created_at
FROM provider_credential_models;

CREATE TABLE request_logs_api_key (
    request_id TEXT PRIMARY KEY,
    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0),
    config_revision INTEGER NOT NULL CHECK (config_revision >= 1),
    gateway_api_key_id TEXT
        REFERENCES gateway_api_keys(id) ON DELETE SET NULL,
    ingress_protocol TEXT NOT NULL CHECK (
        ingress_protocol IN (
            'openai_responses',
            'openai_chat_completions',
            'codex_backend',
            'anthropic_messages'
        )
    ),
    operation TEXT NOT NULL CHECK (
        operation IN (
            'responses',
            'responses_compact',
            'chat_completions',
            'messages',
            'messages_count_tokens'
        )
    ),
    public_model TEXT,
    provider_endpoint_id TEXT
        REFERENCES provider_endpoints(id) ON DELETE SET NULL,
    credential_id TEXT
        REFERENCES provider_credentials_api_key(id) ON DELETE SET NULL,
    proxy_profile_id TEXT
        REFERENCES proxy_profiles(id) ON DELETE SET NULL,
    status_code INTEGER NOT NULL CHECK (status_code BETWEEN 100 AND 599),
    error_class TEXT CHECK (
        error_class IS NULL OR error_class IN (
            'invalid_request', 'authentication', 'permission_denied', 'quota_exhausted',
            'rate_limited', 'model_unavailable', 'operation_unavailable', 'proxy',
            'network', 'upstream', 'cancelled', 'internal'
        )
    ),
    attempt_count INTEGER NOT NULL CHECK (attempt_count >= 0),
    latency_ms INTEGER NOT NULL CHECK (latency_ms >= 0),
    first_token_ms INTEGER CHECK (first_token_ms IS NULL OR first_token_ms >= 0),
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    cache_read_tokens INTEGER CHECK (cache_read_tokens IS NULL OR cache_read_tokens >= 0),
    cache_write_tokens INTEGER CHECK (cache_write_tokens IS NULL OR cache_write_tokens >= 0),
    is_stream INTEGER NOT NULL CHECK (is_stream IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO request_logs_api_key (
    request_id, started_at_ms, config_revision, gateway_api_key_id, ingress_protocol,
    operation, public_model, provider_endpoint_id, credential_id, proxy_profile_id,
    status_code, error_class, attempt_count, latency_ms, first_token_ms, input_tokens,
    output_tokens, cache_read_tokens, cache_write_tokens, is_stream, created_at
)
SELECT
    request_id, started_at_ms, config_revision, gateway_api_key_id, ingress_protocol,
    operation, public_model, provider_endpoint_id, credential_id, proxy_profile_id,
    status_code, error_class, attempt_count, latency_ms, first_token_ms, input_tokens,
    output_tokens, cache_read_tokens, cache_write_tokens, is_stream, created_at
FROM request_logs;

CREATE TABLE request_attempts_api_key (
    request_id TEXT NOT NULL
        REFERENCES request_logs_api_key(request_id) ON DELETE CASCADE,
    attempt_no INTEGER NOT NULL CHECK (attempt_no >= 1),
    route_target_id TEXT
        REFERENCES route_targets(id) ON DELETE SET NULL,
    credential_id TEXT
        REFERENCES provider_credentials_api_key(id) ON DELETE SET NULL,
    proxy_profile_id TEXT
        REFERENCES proxy_profiles(id) ON DELETE SET NULL,
    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0),
    duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0),
    retry_safety TEXT CHECK (
        retry_safety IS NULL OR retry_safety IN (
            'definitely_not_sent', 'rejected_before_execution', 'idempotent', 'ambiguous'
        )
    ),
    error_class TEXT CHECK (
        error_class IS NULL OR error_class IN (
            'invalid_request', 'authentication', 'permission_denied', 'quota_exhausted',
            'rate_limited', 'model_unavailable', 'operation_unavailable', 'proxy',
            'network', 'upstream', 'cancelled', 'internal'
        )
    ),
    status_code INTEGER CHECK (status_code IS NULL OR status_code BETWEEN 100 AND 599),
    outcome TEXT NOT NULL CHECK (
        outcome IN (
            'success', 'transport_error', 'upstream_error', 'invalid_response',
            'local_error', 'stream_error', 'cancelled'
        )
    ),
    PRIMARY KEY (request_id, attempt_no)
);

INSERT INTO request_attempts_api_key (
    request_id, attempt_no, route_target_id, credential_id, proxy_profile_id,
    started_at_ms, duration_ms, retry_safety, error_class, status_code, outcome
)
SELECT
    request_id, attempt_no, route_target_id, credential_id, proxy_profile_id,
    started_at_ms, duration_ms, retry_safety, error_class, status_code, outcome
FROM request_attempts;

DROP TABLE request_attempts;
DROP TABLE request_logs;
DROP TABLE provider_credential_models;
DROP TABLE provider_credentials;

ALTER TABLE provider_credentials_api_key RENAME TO provider_credentials;
ALTER TABLE provider_credential_models_api_key RENAME TO provider_credential_models;
ALTER TABLE request_logs_api_key RENAME TO request_logs;
ALTER TABLE request_attempts_api_key RENAME TO request_attempts;

CREATE INDEX provider_credentials_endpoint_idx
    ON provider_credentials(provider_endpoint_id, enabled);
CREATE INDEX provider_credentials_proxy_idx
    ON provider_credentials(proxy_profile_id);
CREATE INDEX provider_credential_models_model_idx
    ON provider_credential_models(upstream_model, credential_id);
CREATE INDEX request_logs_started_idx
    ON request_logs(started_at_ms DESC, request_id DESC);
CREATE INDEX request_logs_error_idx
    ON request_logs(error_class, started_at_ms DESC);
CREATE INDEX request_logs_gateway_key_started_idx
    ON request_logs(gateway_api_key_id, started_at_ms DESC, request_id DESC);
CREATE INDEX request_attempts_request_idx
    ON request_attempts(request_id, attempt_no);

CREATE TEMP TABLE migration_0017_foreign_key_guard (
    invalid INTEGER NOT NULL CHECK (invalid = 0)
);
INSERT INTO migration_0017_foreign_key_guard (invalid)
SELECT 1 FROM pragma_foreign_key_check;
DROP TABLE migration_0017_foreign_key_guard;
