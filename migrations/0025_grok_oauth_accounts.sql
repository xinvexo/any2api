PRAGMA defer_foreign_keys = ON;

CREATE TABLE oauth_accounts_grok (
    id TEXT PRIMARY KEY,
    provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'claude', 'grok')),
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

INSERT INTO oauth_accounts_grok (
    id, provider_kind, label, label_key, oauth_json, token_version,
    account_generation, config_version, proxy_profile_id, requests_per_minute,
    enabled, safe_account_email, expires_at, created_at, updated_at
)
SELECT
    id, provider_kind, label, label_key, oauth_json, token_version,
    account_generation, config_version, proxy_profile_id, requests_per_minute,
    enabled, safe_account_email, expires_at, created_at, updated_at
FROM oauth_accounts;

CREATE TABLE oauth_account_models_grok (
    oauth_account_id TEXT NOT NULL
        REFERENCES oauth_accounts_grok(id) ON DELETE CASCADE,
    upstream_model TEXT NOT NULL CHECK (
        upstream_model = trim(upstream_model)
        AND length(upstream_model) BETWEEN 1 AND 255
    ),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (oauth_account_id, upstream_model)
);

INSERT INTO oauth_account_models_grok (oauth_account_id, upstream_model, created_at)
SELECT oauth_account_id, upstream_model, created_at
FROM oauth_account_models;

CREATE TABLE request_logs_grok_oauth (
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
        REFERENCES provider_credentials(id) ON DELETE SET NULL,
    oauth_account_id TEXT
        REFERENCES oauth_accounts_grok(id) ON DELETE SET NULL,
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
    error_message TEXT,
    attempt_count INTEGER NOT NULL CHECK (attempt_count >= 0),
    latency_ms INTEGER NOT NULL CHECK (latency_ms >= 0),
    first_token_ms INTEGER CHECK (first_token_ms IS NULL OR first_token_ms >= 0),
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    cache_read_tokens INTEGER CHECK (cache_read_tokens IS NULL OR cache_read_tokens >= 0),
    cache_write_tokens INTEGER CHECK (cache_write_tokens IS NULL OR cache_write_tokens >= 0),
    thinking_level TEXT CHECK (
        thinking_level IS NULL
        OR (
            length(thinking_level) BETWEEN 1 AND 64
            AND thinking_level = trim(thinking_level)
        )
    ),
    is_stream INTEGER NOT NULL CHECK (is_stream IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO request_logs_grok_oauth (
    request_id, started_at_ms, config_revision, gateway_api_key_id, ingress_protocol,
    operation, public_model, provider_endpoint_id, credential_id, oauth_account_id,
    proxy_profile_id, status_code, error_class, error_message, attempt_count, latency_ms,
    first_token_ms, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
    thinking_level, is_stream, created_at
)
SELECT
    request_id, started_at_ms, config_revision, gateway_api_key_id, ingress_protocol,
    operation, public_model, provider_endpoint_id, credential_id, oauth_account_id,
    proxy_profile_id, status_code, error_class, error_message, attempt_count, latency_ms,
    first_token_ms, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
    thinking_level, is_stream, created_at
FROM request_logs;

CREATE TABLE request_attempts_grok_oauth (
    request_id TEXT NOT NULL
        REFERENCES request_logs_grok_oauth(request_id) ON DELETE CASCADE,
    attempt_no INTEGER NOT NULL CHECK (attempt_no >= 1),
    route_target_id TEXT
        REFERENCES route_targets(id) ON DELETE SET NULL,
    credential_id TEXT
        REFERENCES provider_credentials(id) ON DELETE SET NULL,
    oauth_account_id TEXT
        REFERENCES oauth_accounts_grok(id) ON DELETE SET NULL,
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
    error_message TEXT,
    status_code INTEGER CHECK (status_code IS NULL OR status_code BETWEEN 100 AND 599),
    outcome TEXT NOT NULL CHECK (
        outcome IN (
            'success', 'transport_error', 'upstream_error', 'invalid_response',
            'local_error', 'stream_error', 'cancelled'
        )
    ),
    PRIMARY KEY (request_id, attempt_no)
);

INSERT INTO request_attempts_grok_oauth (
    request_id, attempt_no, route_target_id, credential_id, oauth_account_id,
    proxy_profile_id, started_at_ms, duration_ms, retry_safety, error_class,
    error_message, status_code, outcome
)
SELECT
    request_id, attempt_no, route_target_id, credential_id, oauth_account_id,
    proxy_profile_id, started_at_ms, duration_ms, retry_safety, error_class,
    error_message, status_code, outcome
FROM request_attempts;

DROP TABLE request_attempts;
DROP TABLE request_logs;
DROP TABLE oauth_account_models;
DROP TABLE oauth_accounts;

ALTER TABLE oauth_accounts_grok RENAME TO oauth_accounts;
ALTER TABLE oauth_account_models_grok RENAME TO oauth_account_models;
ALTER TABLE request_logs_grok_oauth RENAME TO request_logs;
ALTER TABLE request_attempts_grok_oauth RENAME TO request_attempts;

CREATE INDEX oauth_accounts_provider_enabled_idx
    ON oauth_accounts(provider_kind, enabled);
CREATE INDEX oauth_accounts_expiry_idx
    ON oauth_accounts(enabled, expires_at);
CREATE INDEX oauth_account_models_model_idx
    ON oauth_account_models(upstream_model, oauth_account_id);
CREATE INDEX request_logs_started_idx
    ON request_logs(started_at_ms DESC, request_id DESC);
CREATE INDEX request_logs_error_idx
    ON request_logs(error_class, started_at_ms DESC);
CREATE INDEX request_logs_gateway_key_started_idx
    ON request_logs(gateway_api_key_id, started_at_ms DESC, request_id DESC);
CREATE INDEX request_logs_oauth_account_idx
    ON request_logs(oauth_account_id, started_at_ms DESC);
CREATE INDEX request_logs_provider_credential_started_idx
    ON request_logs(credential_id, started_at_ms DESC, request_id DESC);
CREATE INDEX request_attempts_request_idx
    ON request_attempts(request_id, attempt_no);
CREATE INDEX request_attempts_oauth_account_idx
    ON request_attempts(oauth_account_id, started_at_ms DESC);

CREATE TEMP TABLE migration_0025_foreign_key_guard (
    invalid INTEGER NOT NULL CHECK (invalid = 0)
);
INSERT INTO migration_0025_foreign_key_guard (invalid)
SELECT 1 FROM pragma_foreign_key_check;
DROP TABLE migration_0025_foreign_key_guard;
