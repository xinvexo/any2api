UPDATE provider_credentials
SET credential_generation = credential_generation + 1,
    updated_at = CURRENT_TIMESTAMP
WHERE provider_endpoint_id IN (
    SELECT id
    FROM provider_endpoints
    WHERE provider_kind = 'claude'
      AND substr(base_url, -3) = '/v1'
      AND instr(substr(base_url, instr(base_url, '://') + 3), '/') > 0
);

UPDATE provider_endpoints
SET base_url = substr(base_url, 1, length(base_url) - 3),
    config_version = config_version + 1,
    updated_at = CURRENT_TIMESTAMP
WHERE provider_kind = 'claude'
  AND substr(base_url, -3) = '/v1'
  AND instr(substr(base_url, instr(base_url, '://') + 3), '/') > 0;

UPDATE config_state
SET revision = revision + 1,
    updated_at = CURRENT_TIMESTAMP
WHERE singleton_id = 1
  AND changes() > 0;
