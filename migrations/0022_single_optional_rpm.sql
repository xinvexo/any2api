ALTER TABLE provider_credentials DROP COLUMN max_concurrency;
ALTER TABLE provider_credentials ADD COLUMN requests_per_minute INTEGER CHECK (
    requests_per_minute IS NULL OR requests_per_minute BETWEEN 1 AND 100000
);

ALTER TABLE oauth_accounts DROP COLUMN max_concurrency;
ALTER TABLE oauth_accounts ADD COLUMN requests_per_minute INTEGER CHECK (
    requests_per_minute IS NULL OR requests_per_minute BETWEEN 1 AND 100000
);

ALTER TABLE model_routes
    RENAME COLUMN fallback_on_saturation TO fallback_on_rate_limit;

UPDATE setting_overrides
SET key = 'scheduler.on_rate_limited'
WHERE key = 'scheduler.on_saturated';

UPDATE setting_overrides
SET key = 'scheduler.fallback_on_rate_limit'
WHERE key = 'scheduler.fallback_on_saturation';

DELETE FROM setting_overrides
WHERE key IN (
    'scheduler.auxiliary_global_concurrency',
    'scheduler.auxiliary_per_credential_concurrency'
);
