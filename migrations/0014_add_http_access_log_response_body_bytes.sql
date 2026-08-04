ALTER TABLE http_access_logs
    ADD COLUMN response_body_bytes INTEGER NOT NULL DEFAULT 0
        CHECK (response_body_bytes >= 0);

-- Legacy rows reported the response summary byte count as the captured body
-- total; keep that reading for history written before this column existed.
UPDATE http_access_logs
SET response_body_bytes = response_bytes;
