CREATE TABLE oauth_model_catalog_snapshots (
    provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'claude', 'grok')),
    directory_scope TEXT NOT NULL CHECK (length(directory_scope) BETWEEN 1 AND 96),
    fetched_at INTEGER NOT NULL CHECK (fetched_at >= 0),
    models_json BLOB NOT NULL CHECK (length(models_json) BETWEEN 2 AND 131072),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (provider_kind, directory_scope)
);
