CREATE TEMP TABLE oauth_document_migration_guard (
    valid INTEGER NOT NULL CHECK (valid = 1)
) STRICT;

-- Run JSON functions only after rejecting unreadable rows.
INSERT INTO oauth_document_migration_guard (valid)
SELECT COALESCE(MIN(json_valid(CAST(oauth_json AS TEXT))), 1)
FROM oauth_accounts;

INSERT INTO oauth_document_migration_guard (valid)
SELECT CASE WHEN NOT EXISTS (
    SELECT 1
    FROM oauth_accounts
    WHERE json_type(CAST(oauth_json AS TEXT)) IS NOT 'object'
       OR json_type(CAST(oauth_json AS TEXT), '$.type') IS NOT 'text'
       OR json_extract(CAST(oauth_json AS TEXT), '$.type') <> provider_kind
       OR json_type(CAST(oauth_json AS TEXT), '$.access_token') IS NOT 'text'
       OR length(trim(json_extract(CAST(oauth_json AS TEXT), '$.access_token'))) = 0
) THEN 1 ELSE 0 END;

UPDATE oauth_accounts
SET oauth_json = CAST(json_object(
    'access_token', json_extract(CAST(oauth_json AS TEXT), '$.access_token'),
    'refresh_token', NULLIF(json_extract(CAST(oauth_json AS TEXT), '$.refresh_token'), ''),
    'id_token', NULLIF(json_extract(CAST(oauth_json AS TEXT), '$.id_token'), ''),
    'account_id', COALESCE(
        NULLIF(json_extract(CAST(oauth_json AS TEXT), '$.account_id'), ''),
        NULLIF(json_extract(CAST(oauth_json AS TEXT), '$.sub'), '')
    ),
    'email', COALESCE(
        NULLIF(json_extract(CAST(oauth_json AS TEXT), '$.email'), ''),
        safe_account_email
    )
) AS BLOB);

DROP TABLE oauth_document_migration_guard;
