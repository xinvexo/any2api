ALTER TABLE request_attempts ADD COLUMN routing_mode TEXT CHECK (
    routing_mode IS NULL OR routing_mode IN ('balanced', 'bound')
);

ALTER TABLE request_attempts ADD COLUMN failure_scope TEXT CHECK (
    failure_scope IS NULL OR failure_scope IN (
        'unattributed',
        'authentication',
        'credential',
        'credential_model',
        'route_operation',
        'exact_candidate',
        'egress_path',
        'proxy',
        'endpoint'
    )
);

ALTER TABLE request_attempts ADD COLUMN retry_decision TEXT CHECK (
    retry_decision IS NULL OR retry_decision IN (
        'terminal',
        'oauth_refresh',
        'retry_same_path',
        'reselect'
    )
);
