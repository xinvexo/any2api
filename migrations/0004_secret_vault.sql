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
