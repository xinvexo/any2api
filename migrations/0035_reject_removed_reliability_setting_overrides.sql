-- These controls are now a fixed runtime policy. Existing explicit overrides
-- require operator review instead of being silently ignored or deleted.
SELECT CASE
    WHEN EXISTS (
        SELECT 1
        FROM setting_overrides
        WHERE key IN (
            'retry.max_total_attempts',
            'retry.max_credential_switches',
            'retry.max_same_credential_retries',
            'retry.base_delay',
            'retry.max_delay',
            'retry.jitter_ratio',
            'cooldown.rate_limit_fallback',
            'cooldown.model_unsupported',
            'cooldown.permission_denied',
            'cooldown.transient_endpoint',
            'breaker.endpoint.failure_threshold',
            'breaker.endpoint.failure_window',
            'breaker.endpoint.open_duration',
            'breaker.proxy.failure_threshold',
            'breaker.proxy.failure_window',
            'breaker.proxy.open_duration',
            'breaker.half_open_max_probes'
        )
    )
    THEN abs(-9223372036854775808)
    ELSE 0
END;
