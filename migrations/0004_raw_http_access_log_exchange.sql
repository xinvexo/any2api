ALTER TABLE http_access_logs
    ADD COLUMN uri TEXT NOT NULL DEFAULT '';

ALTER TABLE http_access_logs
    ADD COLUMN exchange_captured INTEGER NOT NULL DEFAULT 0
        CHECK (exchange_captured IN (0, 1));

ALTER TABLE http_access_logs
    ADD COLUMN request_headers BLOB NOT NULL DEFAULT X'5B5D';

ALTER TABLE http_access_logs
    ADD COLUMN request_body BLOB NOT NULL DEFAULT X'';

ALTER TABLE http_access_logs
    ADD COLUMN request_body_bytes INTEGER NOT NULL DEFAULT 0
        CHECK (request_body_bytes >= 0);

ALTER TABLE http_access_logs
    ADD COLUMN request_body_complete INTEGER NOT NULL DEFAULT 0
        CHECK (request_body_complete IN (0, 1));

ALTER TABLE http_access_logs
    ADD COLUMN request_body_truncated INTEGER NOT NULL DEFAULT 0
        CHECK (request_body_truncated IN (0, 1));

ALTER TABLE http_access_logs
    ADD COLUMN response_headers BLOB NOT NULL DEFAULT X'5B5D';

ALTER TABLE http_access_logs
    ADD COLUMN response_body BLOB NOT NULL DEFAULT X'';

ALTER TABLE http_access_logs
    ADD COLUMN response_body_complete INTEGER NOT NULL DEFAULT 0
        CHECK (response_body_complete IN (0, 1));

ALTER TABLE http_access_logs
    ADD COLUMN response_body_truncated INTEGER NOT NULL DEFAULT 0
        CHECK (response_body_truncated IN (0, 1));

UPDATE http_access_logs SET uri = path;
