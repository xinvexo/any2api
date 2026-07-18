CREATE TABLE provider_endpoints (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (name = trim(name) AND length(name) BETWEEN 1 AND 100),
    name_key TEXT NOT NULL UNIQUE,
    provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'claude')),
    base_url TEXT NOT NULL CHECK (
        base_url = trim(base_url)
        AND length(base_url) BETWEEN 1 AND 2048
    ),
    protocol_dialect TEXT NOT NULL CHECK (
        protocol_dialect IN ('openai_responses', 'anthropic_messages')
    ),
    allow_insecure_http INTEGER NOT NULL CHECK (allow_insecure_http IN (0, 1)),
    allow_private_network INTEGER NOT NULL CHECK (allow_private_network IN (0, 1)),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (
        (provider_kind = 'codex' AND protocol_dialect = 'openai_responses')
        OR
        (provider_kind = 'claude' AND protocol_dialect = 'anthropic_messages')
    )
);
