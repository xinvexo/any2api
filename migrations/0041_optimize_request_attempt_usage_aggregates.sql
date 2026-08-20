DROP INDEX request_attempts_credential_idx;
CREATE INDEX request_attempts_credential_idx
    ON request_attempts(credential_id, started_at_ms DESC, outcome, status_code);

DROP INDEX request_attempts_oauth_account_idx;
CREATE INDEX request_attempts_oauth_account_idx
    ON request_attempts(oauth_account_id, started_at_ms DESC, outcome, status_code);
