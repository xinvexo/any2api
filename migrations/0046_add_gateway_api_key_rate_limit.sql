ALTER TABLE gateway_api_keys
ADD COLUMN requests_per_minute INTEGER CHECK (
    requests_per_minute IS NULL OR requests_per_minute BETWEEN 1 AND 100000
);
