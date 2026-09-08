ALTER TABLE request_logs ADD COLUMN quota_credits_per_usd INTEGER
    CHECK (
        quota_credits_per_usd IS NULL
        OR (
            quota_cost_unit IS 'codex_credits'
            AND typeof(quota_credits_per_usd) = 'integer'
            AND quota_credits_per_usd BETWEEN 1 AND 1000000
        )
    );
