CREATE TABLE config_state (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    revision INTEGER NOT NULL CHECK (revision > 0),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO config_state (singleton_id, revision) VALUES (1, 1);

CREATE TABLE setting_overrides (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE proxy_profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (name = trim(name) AND length(name) BETWEEN 1 AND 100),
    name_key TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL CHECK (kind IN ('direct', 'http', 'socks5')),
    host TEXT,
    port INTEGER,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    built_in INTEGER NOT NULL CHECK (built_in IN (0, 1)),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    authentication_version INTEGER NOT NULL DEFAULT 0
        CHECK (authentication_version BETWEEN 0 AND 4294967295),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (
        (
            id = '00000000-0000-0000-0000-000000000000'
            AND name = 'DIRECT'
            AND name_key = 'direct'
            AND kind = 'direct'
            AND host IS NULL
            AND port IS NULL
            AND enabled = 1
            AND built_in = 1
        )
        OR (
            id <> '00000000-0000-0000-0000-000000000000'
            AND kind IN ('http', 'socks5')
            AND host IS NOT NULL
            AND length(host) > 0
            AND port BETWEEN 1 AND 65535
            AND built_in = 0
        )
    )
);

INSERT INTO proxy_profiles (
    id, name, name_key, kind, host, port, enabled, built_in, config_version
) VALUES (
    '00000000-0000-0000-0000-000000000000',
    'DIRECT',
    'direct',
    'direct',
    NULL,
    NULL,
    1,
    1,
    1
);

CREATE TABLE proxy_settings (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    global_proxy_profile_id TEXT NOT NULL
        REFERENCES proxy_profiles(id) ON DELETE RESTRICT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO proxy_settings (singleton_id, global_proxy_profile_id)
VALUES (1, '00000000-0000-0000-0000-000000000000');

CREATE TRIGGER proxy_direct_update_forbidden
BEFORE UPDATE ON proxy_profiles
WHEN OLD.id = '00000000-0000-0000-0000-000000000000'
BEGIN
    SELECT RAISE(ABORT, 'proxy_direct_immutable');
END;

CREATE TRIGGER proxy_direct_delete_forbidden
BEFORE DELETE ON proxy_profiles
WHEN OLD.id = '00000000-0000-0000-0000-000000000000'
BEGIN
    SELECT RAISE(ABORT, 'proxy_direct_immutable');
END;

CREATE TRIGGER proxy_global_disable_forbidden
BEFORE UPDATE OF enabled ON proxy_profiles
WHEN NEW.enabled = 0
    AND EXISTS (
        SELECT 1 FROM proxy_settings
        WHERE singleton_id = 1 AND global_proxy_profile_id = OLD.id
    )
BEGIN
    SELECT RAISE(ABORT, 'proxy_is_global');
END;

CREATE TRIGGER proxy_global_must_be_enabled
BEFORE UPDATE OF global_proxy_profile_id ON proxy_settings
WHEN NOT EXISTS (
    SELECT 1 FROM proxy_profiles
    WHERE id = NEW.global_proxy_profile_id AND enabled = 1
)
BEGIN
    SELECT RAISE(ABORT, 'proxy_global_disabled');
END;

CREATE TRIGGER proxy_global_insert_must_be_enabled
BEFORE INSERT ON proxy_settings
WHEN NOT EXISTS (
    SELECT 1 FROM proxy_profiles
    WHERE id = NEW.global_proxy_profile_id AND enabled = 1
)
BEGIN
    SELECT RAISE(ABORT, 'proxy_global_disabled');
END;

CREATE TRIGGER proxy_settings_delete_forbidden
BEFORE DELETE ON proxy_settings
BEGIN
    SELECT RAISE(ABORT, 'proxy_settings_immutable');
END;

CREATE TABLE secret_vault_metadata (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    envelope_version INTEGER NOT NULL CHECK (envelope_version = 1),
    key_id TEXT NOT NULL CHECK (length(key_id) BETWEEN 1 AND 128),
    algorithm TEXT NOT NULL CHECK (algorithm = 'xchacha20poly1305'),
    nonce BLOB NOT NULL CHECK (length(nonce) = 24),
    ciphertext BLOB NOT NULL CHECK (length(ciphertext) >= 16),
    aad_version INTEGER NOT NULL CHECK (aad_version = 1),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER secret_vault_metadata_prevent_delete
BEFORE DELETE ON secret_vault_metadata
BEGIN
    SELECT RAISE(ABORT, 'secret vault metadata cannot be deleted');
END;

CREATE TRIGGER secret_vault_metadata_prevent_identity_update
BEFORE UPDATE OF singleton_id ON secret_vault_metadata
BEGIN
    SELECT RAISE(ABORT, 'secret vault metadata identity cannot change');
END;

CREATE TABLE admin_credentials (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    password_hash TEXT NOT NULL CHECK (length(password_hash) BETWEEN 1 AND 512),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE gateway_api_keys (
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
    hash_version INTEGER NOT NULL CHECK (hash_version = 1),
    hash_key_id TEXT NOT NULL CHECK (length(hash_key_id) BETWEEN 1 AND 128),
    token_version INTEGER NOT NULL CHECK (token_version BETWEEN 1 AND 4294967295),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX gateway_api_keys_enabled_name_idx
    ON gateway_api_keys(enabled, name);

CREATE TABLE proxy_passwords (
    proxy_profile_id TEXT PRIMARY KEY
        REFERENCES proxy_profiles(id) ON DELETE CASCADE,
    username TEXT NOT NULL CHECK (length(username) BETWEEN 1 AND 255),
    authentication_version INTEGER NOT NULL
        CHECK (authentication_version BETWEEN 1 AND 4294967295),
    envelope_version INTEGER NOT NULL CHECK (envelope_version = 1),
    key_id TEXT NOT NULL CHECK (length(key_id) BETWEEN 1 AND 128),
    algorithm TEXT NOT NULL CHECK (algorithm = 'xchacha20poly1305'),
    nonce BLOB NOT NULL CHECK (length(nonce) = 24),
    ciphertext BLOB NOT NULL CHECK (length(ciphertext) >= 16),
    aad_version INTEGER NOT NULL CHECK (aad_version = 1),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (proxy_profile_id <> '00000000-0000-0000-0000-000000000000')
);

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

CREATE TABLE provider_endpoints (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (name = trim(name) AND length(name) BETWEEN 1 AND 100),
    name_key TEXT NOT NULL UNIQUE,
    provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'claude', 'grok')),
    base_url TEXT NOT NULL CHECK (
        base_url = trim(base_url) AND length(base_url) BETWEEN 1 AND 2048
    ),
    protocol_dialect TEXT NOT NULL CHECK (
        protocol_dialect IN (
            'openai_responses',
            'openai_chat_completions',
            'openai_images',
            'anthropic_messages'
        )
    ),
    upstream_protocol_dialect TEXT CHECK (
        upstream_protocol_dialect IS NULL
        OR (
            upstream_protocol_dialect IN (
                'openai_responses',
                'openai_chat_completions',
                'openai_images',
                'anthropic_messages'
            )
            AND upstream_protocol_dialect <> protocol_dialect
        )
    ),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE provider_credentials (
    id TEXT PRIMARY KEY,
    provider_endpoint_id TEXT NOT NULL
        REFERENCES provider_endpoints(id) ON DELETE RESTRICT,
    label TEXT NOT NULL CHECK (label = trim(label) AND length(label) BETWEEN 1 AND 100),
    label_key TEXT NOT NULL,
    credential_kind TEXT NOT NULL CHECK (credential_kind = 'api_key'),
    secret_schema_version INTEGER NOT NULL CHECK (secret_schema_version = 1),
    secret_version INTEGER NOT NULL CHECK (secret_version BETWEEN 1 AND 4294967295),
    credential_generation INTEGER NOT NULL
        CHECK (credential_generation BETWEEN 1 AND 4294967295),
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
    requests_per_minute INTEGER CHECK (
        requests_per_minute IS NULL OR requests_per_minute BETWEEN 1 AND 100000
    ),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(provider_endpoint_id, label_key)
);

CREATE INDEX provider_credentials_endpoint_idx
    ON provider_credentials(provider_endpoint_id, enabled);
CREATE INDEX provider_credentials_proxy_idx
    ON provider_credentials(proxy_profile_id);

CREATE TABLE provider_credential_models (
    credential_id TEXT NOT NULL
        REFERENCES provider_credentials(id) ON DELETE CASCADE,
    upstream_model TEXT NOT NULL CHECK (
        upstream_model = trim(upstream_model)
        AND length(upstream_model) BETWEEN 1 AND 255
    ),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (credential_id, upstream_model)
);

CREATE INDEX provider_credential_models_model_idx
    ON provider_credential_models(upstream_model, credential_id);

CREATE TABLE oauth_accounts (
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

CREATE INDEX oauth_accounts_provider_enabled_idx
    ON oauth_accounts(provider_kind, enabled);
CREATE INDEX oauth_accounts_expiry_idx
    ON oauth_accounts(enabled, expires_at);

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

CREATE INDEX oauth_account_models_model_idx
    ON oauth_account_models(upstream_model, oauth_account_id);

CREATE TABLE model_routes (
    id TEXT PRIMARY KEY,
    public_model TEXT NOT NULL CHECK (
        public_model = trim(public_model) AND length(public_model) BETWEEN 1 AND 255
    ),
    ingress_protocol TEXT NOT NULL CHECK (
        ingress_protocol IN (
            'openai_responses',
            'openai_chat_completions',
            'openai_images',
            'anthropic_messages'
        )
    ),
    fallback_on_rate_limit INTEGER CHECK (
        fallback_on_rate_limit IS NULL OR fallback_on_rate_limit IN (0, 1)
    ),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(ingress_protocol, public_model)
);

CREATE TABLE route_targets (
    id TEXT PRIMARY KEY,
    model_route_id TEXT NOT NULL REFERENCES model_routes(id) ON DELETE CASCADE,
    provider_endpoint_id TEXT NOT NULL
        REFERENCES provider_endpoints(id) ON DELETE RESTRICT,
    upstream_model TEXT NOT NULL CHECK (
        upstream_model = trim(upstream_model)
        AND length(upstream_model) BETWEEN 1 AND 255
    ),
    upstream_protocol_dialect TEXT NOT NULL CHECK (
        upstream_protocol_dialect IN (
            'openai_responses',
            'openai_chat_completions',
            'openai_images',
            'anthropic_messages'
        )
    ),
    fallback_tier INTEGER NOT NULL CHECK (fallback_tier BETWEEN 0 AND 65535),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (
        model_route_id,
        provider_endpoint_id,
        upstream_model,
        upstream_protocol_dialect
    )
);

CREATE INDEX route_targets_route_tier_idx
    ON route_targets(model_route_id, fallback_tier, enabled);
CREATE INDEX route_targets_endpoint_idx
    ON route_targets(provider_endpoint_id);

CREATE TABLE request_logs (
    request_id TEXT PRIMARY KEY,
    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0),
    config_revision INTEGER NOT NULL CHECK (config_revision >= 1),
    gateway_api_key_id TEXT REFERENCES gateway_api_keys(id) ON DELETE SET NULL,
    ingress_protocol TEXT NOT NULL CHECK (
        ingress_protocol IN (
            'openai_responses',
            'openai_chat_completions',
            'openai_images',
            'anthropic_messages'
        )
    ),
    operation TEXT NOT NULL CHECK (
        operation IN (
            'responses',
            'responses_compact',
            'chat_completions',
            'images_generations',
            'images_edits',
            'messages',
            'messages_count_tokens'
        )
    ),
    public_model TEXT,
    provider_endpoint_id TEXT REFERENCES provider_endpoints(id) ON DELETE SET NULL,
    credential_id TEXT REFERENCES provider_credentials(id) ON DELETE SET NULL,
    oauth_account_id TEXT REFERENCES oauth_accounts(id) ON DELETE SET NULL,
    proxy_profile_id TEXT REFERENCES proxy_profiles(id) ON DELETE SET NULL,
    status_code INTEGER NOT NULL CHECK (status_code BETWEEN 100 AND 599),
    error_class TEXT CHECK (
        error_class IS NULL OR error_class IN (
            'invalid_request',
            'authentication',
            'permission_denied',
            'quota_exhausted',
            'rate_limited',
            'model_unavailable',
            'operation_unavailable',
            'proxy',
            'network',
            'upstream',
            'cancelled',
            'internal'
        )
    ),
    error_message TEXT,
    attempt_count INTEGER NOT NULL CHECK (attempt_count >= 0),
    latency_ms INTEGER NOT NULL CHECK (latency_ms >= 0),
    first_token_ms INTEGER CHECK (first_token_ms IS NULL OR first_token_ms >= 0),
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    cache_read_tokens INTEGER CHECK (cache_read_tokens IS NULL OR cache_read_tokens >= 0),
    thinking_level TEXT CHECK (
        thinking_level IS NULL
        OR (
            length(thinking_level) BETWEEN 1 AND 64
            AND thinking_level = trim(thinking_level)
        )
    ),
    is_stream INTEGER NOT NULL CHECK (is_stream IN (0, 1)),
    client_ip TEXT NOT NULL CHECK (
        typeof(client_ip) = 'text'
        AND client_ip = trim(client_ip)
        AND length(client_ip) BETWEEN 2 AND 45
    ),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

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

CREATE TABLE request_attempts (
    request_id TEXT NOT NULL REFERENCES request_logs(request_id) ON DELETE CASCADE,
    attempt_no INTEGER NOT NULL CHECK (attempt_no >= 1),
    route_target_id TEXT REFERENCES route_targets(id) ON DELETE SET NULL,
    credential_id TEXT REFERENCES provider_credentials(id) ON DELETE SET NULL,
    oauth_account_id TEXT REFERENCES oauth_accounts(id) ON DELETE SET NULL,
    proxy_profile_id TEXT REFERENCES proxy_profiles(id) ON DELETE SET NULL,
    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0),
    duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0),
    retry_safety TEXT CHECK (
        retry_safety IS NULL OR retry_safety IN (
            'definitely_not_sent',
            'rejected_before_execution',
            'idempotent',
            'ambiguous'
        )
    ),
    error_class TEXT CHECK (
        error_class IS NULL OR error_class IN (
            'invalid_request',
            'authentication',
            'permission_denied',
            'quota_exhausted',
            'rate_limited',
            'model_unavailable',
            'operation_unavailable',
            'proxy',
            'network',
            'upstream',
            'cancelled',
            'internal'
        )
    ),
    error_message TEXT,
    status_code INTEGER CHECK (status_code IS NULL OR status_code BETWEEN 100 AND 599),
    outcome TEXT NOT NULL CHECK (
        outcome IN (
            'success',
            'transport_error',
            'upstream_error',
            'invalid_response',
            'local_error',
            'stream_error',
            'cancelled'
        )
    ),
    PRIMARY KEY (request_id, attempt_no)
);

CREATE INDEX request_attempts_request_idx
    ON request_attempts(request_id, attempt_no);
CREATE INDEX request_attempts_oauth_account_idx
    ON request_attempts(oauth_account_id, started_at_ms DESC);

CREATE TABLE http_access_logs (
    request_id TEXT PRIMARY KEY NOT NULL,
    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0),
    config_revision INTEGER NOT NULL CHECK (config_revision >= 1),
    client_ip TEXT CHECK (
        client_ip IS NULL
        OR (
            typeof(client_ip) = 'text'
            AND client_ip = trim(client_ip)
            AND length(client_ip) BETWEEN 2 AND 45
        )
    ),
    method TEXT NOT NULL CHECK (
        typeof(method) = 'text' AND length(method) >= 1 AND method = trim(method)
    ),
    path TEXT NOT NULL CHECK (typeof(path) = 'text' AND length(path) >= 1),
    http_version TEXT NOT NULL CHECK (
        http_version IN ('HTTP/0.9', 'HTTP/1.0', 'HTTP/1.1', 'HTTP/2', 'HTTP/3')
    ),
    status_code INTEGER CHECK (status_code IS NULL OR status_code BETWEEN 100 AND 999),
    duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0),
    response_bytes INTEGER NOT NULL CHECK (response_bytes >= 0),
    outcome TEXT NOT NULL CHECK (outcome IN ('completed', 'body_error', 'cancelled'))
) STRICT;

CREATE INDEX http_access_logs_started_idx
    ON http_access_logs(started_at_ms DESC, request_id DESC);
