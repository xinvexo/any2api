ALTER TABLE provider_credential_models ADD COLUMN public_model TEXT CHECK (
    public_model IS NULL OR (
        public_model = trim(public_model)
        AND length(public_model) BETWEEN 1 AND 255
        AND public_model <> upstream_model
    )
);

CREATE UNIQUE INDEX provider_credential_models_public_idx
    ON provider_credential_models(credential_id, COALESCE(public_model, upstream_model));
