ALTER TABLE request_logs
    ADD COLUMN oauth_account_id TEXT
    REFERENCES oauth_accounts(id) ON DELETE SET NULL;

ALTER TABLE request_attempts
    ADD COLUMN oauth_account_id TEXT
    REFERENCES oauth_accounts(id) ON DELETE SET NULL;

CREATE INDEX request_logs_oauth_account_idx
    ON request_logs(oauth_account_id, started_at_ms DESC);

CREATE INDEX request_attempts_oauth_account_idx
    ON request_attempts(oauth_account_id, started_at_ms DESC);
