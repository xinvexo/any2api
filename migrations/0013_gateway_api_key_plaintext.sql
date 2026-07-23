-- Hash-only rows cannot recover displayable tokens; wipe them for the plaintext model.
-- Soft-revoked keys are also purged; new deletes are physical.
DELETE FROM gateway_api_keys;

ALTER TABLE gateway_api_keys ADD COLUMN token TEXT NOT NULL DEFAULT '';
