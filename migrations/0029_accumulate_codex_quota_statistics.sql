-- ADR-0146: collapse the v8 sample list into cumulative local facts. Runtime
-- reads only this v9 shape; the old fields exist solely at this SQL boundary.
CREATE TABLE oauth_quota_snapshots_v9 (
    oauth_account_id TEXT PRIMARY KEY NOT NULL
        REFERENCES oauth_accounts(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL CHECK (schema_version = 9),
    fetched_at INTEGER NOT NULL CHECK (fetched_at >= 0),
    payload BLOB NOT NULL CHECK (
        typeof(payload) = 'blob' AND length(payload) BETWEEN 2 AND 524288
    ),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

INSERT INTO oauth_quota_snapshots_v9 (
    oauth_account_id,
    schema_version,
    fetched_at,
    payload,
    updated_at
)
SELECT
    snapshot.oauth_account_id,
    9,
    snapshot.fetched_at,
    CAST(
        json_object(
            'usage', json(json_extract(CAST(snapshot.payload AS TEXT), '$.usage')),
            'estimator_state',
            CASE
                WHEN json_type(CAST(snapshot.payload AS TEXT), '$.estimator_state') IS NULL
                    OR json_type(CAST(snapshot.payload AS TEXT), '$.estimator_state') = 'null'
                THEN NULL
                ELSE json_object(
                    'credential_fingerprint', json_extract(
                        CAST(snapshot.payload AS TEXT),
                        '$.estimator_state.credential_fingerprint'
                    ),
                    'subscription_tier', json_extract(
                        CAST(snapshot.payload AS TEXT),
                        '$.estimator_state.subscription_tier'
                    ),
                    'windows', json(COALESCE((
                        SELECT json_group_array(json_object(
                            'key', json(json_extract(window.value, '$.key')),
                            'anchor', json_object(
                                'used_percent', json_extract(
                                    window.value,
                                    '$.sample_anchor.used_percent'
                                ),
                                'reset_at', json_extract(
                                    window.value,
                                    '$.sample_anchor.reset_at'
                                ),
                                'position', json(json_extract(
                                    window.value,
                                    '$.sample_anchor.telemetry.position'
                                ))
                            ),
                            'total_delta_used_percent', COALESCE((
                                SELECT SUM(CAST(json_extract(
                                    sample.value,
                                    '$.delta_used_percent'
                                ) AS REAL))
                                FROM json_each(window.value, '$.samples') AS sample
                            ), 0.0),
                            'total_local_cost_credits', COALESCE((
                                SELECT SUM(CAST(json_extract(
                                    sample.value,
                                    '$.local_cost_credits'
                                ) AS REAL))
                                FROM json_each(window.value, '$.samples') AS sample
                            ), 0.0),
                            'completed_interval_count', (
                                SELECT COUNT(*)
                                FROM json_each(window.value, '$.samples') AS sample
                            )
                        ))
                        FROM json_each(
                            CAST(snapshot.payload AS TEXT),
                            '$.estimator_state.windows'
                        ) AS window
                    ), '[]'))
                )
            END
        ) AS BLOB
    ),
    snapshot.updated_at
FROM oauth_quota_snapshots AS snapshot;

DROP TABLE oauth_quota_snapshots;
ALTER TABLE oauth_quota_snapshots_v9 RENAME TO oauth_quota_snapshots;
