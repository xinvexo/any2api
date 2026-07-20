CREATE TABLE request_logs (
    request_id TEXT PRIMARY KEY,
    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0),
    config_revision INTEGER NOT NULL CHECK (config_revision >= 1),
    gateway_api_key_id TEXT
        REFERENCES gateway_api_keys(id) ON DELETE SET NULL,
    ingress_protocol TEXT NOT NULL CHECK (
        ingress_protocol IN ('openai_responses', 'codex_backend', 'anthropic_messages')
    ),
    operation TEXT NOT NULL CHECK (
        operation IN ('responses', 'responses_compact', 'messages', 'messages_count_tokens')
    ),
    public_model TEXT,
    provider_endpoint_id TEXT
        REFERENCES provider_endpoints(id) ON DELETE SET NULL,
    credential_id TEXT
        REFERENCES provider_credentials(id) ON DELETE SET NULL,
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

CREATE TABLE request_attempts (
    request_id TEXT NOT NULL
        REFERENCES request_logs(request_id) ON DELETE CASCADE,
    attempt_no INTEGER NOT NULL CHECK (attempt_no >= 1),
    route_target_id TEXT
        REFERENCES route_targets(id) ON DELETE SET NULL,
    credential_id TEXT
        REFERENCES provider_credentials(id) ON DELETE SET NULL,
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

CREATE INDEX request_logs_started_idx
    ON request_logs(started_at_ms DESC, request_id DESC);

CREATE INDEX request_logs_error_idx
    ON request_logs(error_class, started_at_ms DESC);

CREATE INDEX request_attempts_request_idx
    ON request_attempts(request_id, attempt_no);
