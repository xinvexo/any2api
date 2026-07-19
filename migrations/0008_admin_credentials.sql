CREATE TABLE admin_credentials (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    password_hash TEXT NOT NULL CHECK (length(password_hash) BETWEEN 1 AND 512),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
