ALTER TABLE proxy_profiles
ADD COLUMN authentication_version INTEGER NOT NULL DEFAULT 0
    CHECK (authentication_version BETWEEN 0 AND 4294967295);

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
    SELECT 1
    FROM proxy_profiles
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
    SELECT 1
    FROM proxy_profiles
    WHERE id = NEW.proxy_profile_id
      AND kind IN ('http', 'socks5')
      AND authentication_version = NEW.authentication_version
)
BEGIN
    SELECT RAISE(ABORT, 'proxy_password_version_mismatch');
END;
