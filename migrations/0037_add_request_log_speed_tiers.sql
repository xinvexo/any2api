ALTER TABLE request_logs
ADD COLUMN requested_speed_tier TEXT
CHECK (
    requested_speed_tier IS NULL
    OR requested_speed_tier IN ('standard', 'fast')
);

ALTER TABLE request_logs
ADD COLUMN effective_speed_tier TEXT
CHECK (
    effective_speed_tier IS NULL
    OR effective_speed_tier IN ('standard', 'fast')
);
