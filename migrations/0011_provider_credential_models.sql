CREATE TABLE provider_credential_models (
    credential_id TEXT NOT NULL
        REFERENCES provider_credentials(id) ON DELETE CASCADE,
    upstream_model TEXT NOT NULL CHECK (
        upstream_model = trim(upstream_model)
        AND length(upstream_model) BETWEEN 1 AND 255
    ),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (credential_id, upstream_model)
);

CREATE INDEX provider_credential_models_model_idx
    ON provider_credential_models(upstream_model, credential_id);

INSERT OR IGNORE INTO provider_credential_models (credential_id, upstream_model)
SELECT credentials.id, targets.upstream_model
FROM provider_credentials AS credentials
JOIN route_targets AS targets
  ON targets.provider_endpoint_id = credentials.provider_endpoint_id
JOIN model_routes AS routes
  ON routes.id = targets.model_route_id
WHERE routes.enabled = 1
  AND targets.enabled = 1;
