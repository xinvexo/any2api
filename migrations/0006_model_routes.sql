CREATE TABLE model_routes (
    id TEXT PRIMARY KEY,
    public_model TEXT NOT NULL CHECK (
        public_model = trim(public_model)
        AND length(public_model) BETWEEN 1 AND 255
    ),
    ingress_protocol TEXT NOT NULL CHECK (
        ingress_protocol IN ('openai_responses', 'anthropic_messages')
    ),
    fallback_on_saturation INTEGER CHECK (
        fallback_on_saturation IS NULL OR fallback_on_saturation IN (0, 1)
    ),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    config_version INTEGER NOT NULL CHECK (config_version BETWEEN 1 AND 4294967295),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(ingress_protocol, public_model)
);

CREATE TABLE route_targets (
    id TEXT PRIMARY KEY,
    model_route_id TEXT NOT NULL
        REFERENCES model_routes(id) ON DELETE CASCADE,
    provider_endpoint_id TEXT NOT NULL
        REFERENCES provider_endpoints(id) ON DELETE RESTRICT,
    upstream_model TEXT NOT NULL CHECK (
        upstream_model = trim(upstream_model)
        AND length(upstream_model) BETWEEN 1 AND 255
    ),
    fallback_tier INTEGER NOT NULL CHECK (fallback_tier BETWEEN 0 AND 65535),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(model_route_id, provider_endpoint_id, upstream_model)
);

CREATE INDEX route_targets_route_tier_idx
    ON route_targets(model_route_id, fallback_tier, enabled);

CREATE INDEX route_targets_endpoint_idx
    ON route_targets(provider_endpoint_id);
