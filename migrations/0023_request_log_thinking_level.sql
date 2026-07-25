ALTER TABLE request_logs
    ADD COLUMN thinking_level TEXT
    CHECK (
        thinking_level IS NULL
        OR (
            length(thinking_level) BETWEEN 1 AND 64
            AND thinking_level = trim(thinking_level)
        )
    );
