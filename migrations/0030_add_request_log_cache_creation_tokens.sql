ALTER TABLE request_logs
    ADD COLUMN cache_creation_tokens INTEGER
        CHECK (cache_creation_tokens IS NULL OR cache_creation_tokens >= 0);
