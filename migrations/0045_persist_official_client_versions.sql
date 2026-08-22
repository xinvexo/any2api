CREATE TABLE official_client_versions (
    provider_kind TEXT PRIMARY KEY NOT NULL CHECK (
        provider_kind = trim(provider_kind)
        AND length(provider_kind) BETWEEN 1 AND 64
    ),
    version TEXT NOT NULL CHECK (
        version = trim(version)
        AND length(version) BETWEEN 1 AND 64
    ),
    fetched_at INTEGER NOT NULL CHECK (fetched_at >= 0),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;
