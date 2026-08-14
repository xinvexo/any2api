CREATE TABLE request_logs_v32 (
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
            'alpha_search',
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
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    quota_cost_unit TEXT,
    quota_cost_nanos INTEGER,
    quota_cost_rate_card TEXT,
    quota_service_tier TEXT CHECK (
        (
            quota_cost_unit IS NULL
            AND quota_cost_nanos IS NULL
            AND quota_cost_rate_card IS NULL
            AND quota_service_tier IS NULL
        )
        OR (
            quota_cost_unit = 'codex_credits'
            AND quota_cost_nanos >= 0
            AND typeof(quota_cost_nanos) = 'integer'
            AND length(quota_cost_rate_card) BETWEEN 1 AND 128
            AND quota_service_tier IN ('standard', 'fast')
        )
    ),
    telemetry_process_id TEXT,
    telemetry_sequence INTEGER CHECK (
        (
            telemetry_process_id IS NULL
            AND telemetry_sequence IS NULL
        )
        OR (
            telemetry_process_id IS NOT NULL
            AND length(telemetry_process_id) = 36
            AND telemetry_sequence >= 1
            AND typeof(telemetry_sequence) = 'integer'
        )
    ),
    cache_creation_tokens INTEGER
        CHECK (cache_creation_tokens IS NULL OR cache_creation_tokens >= 0)
);

INSERT INTO request_logs_v32 (
    request_id,
    started_at_ms,
    config_revision,
    gateway_api_key_id,
    ingress_protocol,
    operation,
    public_model,
    provider_endpoint_id,
    credential_id,
    oauth_account_id,
    proxy_profile_id,
    status_code,
    error_class,
    error_message,
    attempt_count,
    latency_ms,
    first_token_ms,
    input_tokens,
    output_tokens,
    cache_read_tokens,
    thinking_level,
    is_stream,
    client_ip,
    created_at,
    quota_cost_unit,
    quota_cost_nanos,
    quota_cost_rate_card,
    quota_service_tier,
    telemetry_process_id,
    telemetry_sequence,
    cache_creation_tokens
)
SELECT
    request_id,
    started_at_ms,
    config_revision,
    gateway_api_key_id,
    ingress_protocol,
    operation,
    public_model,
    provider_endpoint_id,
    credential_id,
    oauth_account_id,
    proxy_profile_id,
    status_code,
    error_class,
    error_message,
    attempt_count,
    latency_ms,
    first_token_ms,
    input_tokens,
    output_tokens,
    cache_read_tokens,
    thinking_level,
    is_stream,
    client_ip,
    created_at,
    quota_cost_unit,
    quota_cost_nanos,
    quota_cost_rate_card,
    quota_service_tier,
    telemetry_process_id,
    telemetry_sequence,
    cache_creation_tokens
FROM request_logs;

DROP TABLE request_logs;
ALTER TABLE request_logs_v32 RENAME TO request_logs;

CREATE INDEX request_logs_started_idx
    ON request_logs(started_at_ms DESC, request_id DESC);
CREATE INDEX request_logs_error_idx
    ON request_logs(error_class, started_at_ms DESC);
CREATE INDEX request_logs_provider_endpoint_idx
    ON request_logs(provider_endpoint_id);
CREATE INDEX request_logs_proxy_profile_idx
    ON request_logs(proxy_profile_id);
CREATE INDEX request_logs_gateway_key_started_idx
    ON request_logs(
        gateway_api_key_id,
        started_at_ms DESC,
        request_id DESC,
        status_code,
        error_class
    );
CREATE INDEX request_logs_provider_credential_started_idx
    ON request_logs(
        credential_id,
        started_at_ms DESC,
        request_id DESC,
        status_code,
        error_class
    );
CREATE INDEX request_logs_oauth_account_idx
    ON request_logs(
        oauth_account_id,
        started_at_ms DESC,
        status_code,
        error_class
    );
CREATE UNIQUE INDEX request_logs_telemetry_position_idx
    ON request_logs(telemetry_process_id, telemetry_sequence)
    WHERE telemetry_process_id IS NOT NULL;
CREATE INDEX request_logs_oauth_quota_sequence_idx
    ON request_logs(oauth_account_id, telemetry_process_id, telemetry_sequence)
    WHERE oauth_account_id IS NOT NULL AND telemetry_process_id IS NOT NULL;

CREATE TRIGGER request_logs_capacity_stats_insert
AFTER INSERT ON request_logs
BEGIN
    UPDATE telemetry_capacity_stats
    SET request_log_rows = request_log_rows + 1
    WHERE singleton_id = 1;
END;

CREATE TRIGGER request_logs_capacity_stats_delete
AFTER DELETE ON request_logs
BEGIN
    UPDATE telemetry_capacity_stats
    SET request_log_rows = request_log_rows - 1
    WHERE singleton_id = 1;
END;
