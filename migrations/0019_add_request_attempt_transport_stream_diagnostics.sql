ALTER TABLE request_attempts ADD COLUMN transport_wire_profile_id TEXT CHECK (
    transport_wire_profile_id IS NULL OR
    length(transport_wire_profile_id) BETWEEN 1 AND 64
);

ALTER TABLE request_attempts ADD COLUMN transport_wire_profile_version INTEGER CHECK (
    transport_wire_profile_version IS NULL OR
    transport_wire_profile_version BETWEEN 1 AND 65535
);

ALTER TABLE request_attempts ADD COLUMN transport_timeout_policy_version INTEGER CHECK (
    transport_timeout_policy_version IS NULL OR
    transport_timeout_policy_version BETWEEN 1 AND 65535
);

ALTER TABLE request_attempts ADD COLUMN transport_resolver_mode TEXT CHECK (
    transport_resolver_mode IS NULL OR
    transport_resolver_mode IN ('system', 'proxy_remote', 'local_cached')
);

ALTER TABLE request_attempts ADD COLUMN transport_proxy_kind TEXT CHECK (
    transport_proxy_kind IS NULL OR
    transport_proxy_kind IN ('direct', 'http', 'socks5')
);

ALTER TABLE request_attempts ADD COLUMN transport_connect_timeout_ms INTEGER CHECK (
    transport_connect_timeout_ms IS NULL OR transport_connect_timeout_ms >= 0
);

ALTER TABLE request_attempts ADD COLUMN transport_read_timeout_ms INTEGER CHECK (
    transport_read_timeout_ms IS NULL OR transport_read_timeout_ms >= 0
);

ALTER TABLE request_attempts ADD COLUMN transport_pool_idle_timeout_ms INTEGER CHECK (
    transport_pool_idle_timeout_ms IS NULL OR transport_pool_idle_timeout_ms >= 0
);

ALTER TABLE request_attempts ADD COLUMN transport_routing_generation INTEGER CHECK (
    transport_routing_generation IS NULL OR transport_routing_generation >= 1
);

ALTER TABLE request_attempts ADD COLUMN transport_authentication_version INTEGER CHECK (
    transport_authentication_version IS NULL OR transport_authentication_version >= 1
);

ALTER TABLE request_attempts ADD COLUMN transport_traffic_class TEXT CHECK (
    (
        transport_traffic_class IS NULL AND
        transport_wire_profile_id IS NULL AND
        transport_wire_profile_version IS NULL AND
        transport_timeout_policy_version IS NULL AND
        transport_resolver_mode IS NULL AND
        transport_proxy_kind IS NULL AND
        transport_connect_timeout_ms IS NULL AND
        transport_read_timeout_ms IS NULL AND
        transport_pool_idle_timeout_ms IS NULL AND
        transport_routing_generation IS NULL AND
        transport_authentication_version IS NULL
    ) OR (
        transport_traffic_class IN ('data_plane', 'oauth_token', 'oauth_quota', 'diagnostic') AND
        transport_wire_profile_id IS NOT NULL AND
        transport_wire_profile_version IS NOT NULL AND
        transport_timeout_policy_version IS NOT NULL AND
        transport_resolver_mode IS NOT NULL AND
        transport_proxy_kind IS NOT NULL AND
        transport_connect_timeout_ms IS NOT NULL AND
        transport_read_timeout_ms IS NOT NULL AND
        transport_pool_idle_timeout_ms IS NOT NULL AND
        transport_routing_generation IS NOT NULL AND
        transport_authentication_version IS NOT NULL
    )
);

ALTER TABLE request_attempts ADD COLUMN first_upstream_frame_ms INTEGER CHECK (
    first_upstream_frame_ms IS NULL OR first_upstream_frame_ms >= 0
);

ALTER TABLE request_attempts ADD COLUMN stream_commit_ms INTEGER CHECK (
    stream_commit_ms IS NULL OR stream_commit_ms >= 0
);

ALTER TABLE request_attempts ADD COLUMN first_downstream_byte_ms INTEGER CHECK (
    first_downstream_byte_ms IS NULL OR first_downstream_byte_ms >= 0
);

ALTER TABLE request_attempts ADD COLUMN stream_cancel_ms INTEGER CHECK (
    stream_cancel_ms IS NULL OR stream_cancel_ms >= 0
);
