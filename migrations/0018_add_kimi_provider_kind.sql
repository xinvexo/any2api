CREATE TABLE provider_endpoints_v18 (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (name = trim(name) AND length(name) BETWEEN 1 AND 100),
    name_key TEXT NOT NULL UNIQUE,
    provider_kind TEXT NOT NULL CHECK (
        provider_kind IN ('codex', 'claude', 'grok', 'kimi')
    ),
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

INSERT INTO provider_endpoints_v18 (
    id,
    name,
    name_key,
    provider_kind,
    base_url,
    protocol_dialect,
    upstream_protocol_dialect,
    enabled,
    config_version,
    created_at,
    updated_at
)
SELECT
    id,
    name,
    name_key,
    provider_kind,
    base_url,
    protocol_dialect,
    upstream_protocol_dialect,
    enabled,
    config_version,
    created_at,
    updated_at
FROM provider_endpoints;

DROP TABLE provider_endpoints;
ALTER TABLE provider_endpoints_v18 RENAME TO provider_endpoints;
