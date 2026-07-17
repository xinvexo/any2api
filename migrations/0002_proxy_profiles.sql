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
        OR
        (
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
    id,
    name,
    name_key,
    kind,
    host,
    port,
    enabled,
    built_in,
    config_version
)
VALUES (
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
    global_proxy_profile_id TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (global_proxy_profile_id) REFERENCES proxy_profiles(id) ON DELETE RESTRICT
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
        SELECT 1
        FROM proxy_settings
        WHERE singleton_id = 1
          AND global_proxy_profile_id = OLD.id
    )
BEGIN
    SELECT RAISE(ABORT, 'proxy_is_global');
END;

CREATE TRIGGER proxy_global_must_be_enabled
BEFORE UPDATE OF global_proxy_profile_id ON proxy_settings
WHEN NOT EXISTS (
    SELECT 1
    FROM proxy_profiles
    WHERE id = NEW.global_proxy_profile_id
      AND enabled = 1
)
BEGIN
    SELECT RAISE(ABORT, 'proxy_global_disabled');
END;

CREATE TRIGGER proxy_global_insert_must_be_enabled
BEFORE INSERT ON proxy_settings
WHEN NOT EXISTS (
    SELECT 1
    FROM proxy_profiles
    WHERE id = NEW.global_proxy_profile_id
      AND enabled = 1
)
BEGIN
    SELECT RAISE(ABORT, 'proxy_global_disabled');
END;

CREATE TRIGGER proxy_settings_delete_forbidden
BEFORE DELETE ON proxy_settings
BEGIN
    SELECT RAISE(ABORT, 'proxy_settings_immutable');
END;
