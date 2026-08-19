-- Previous quota fingerprints always included account_generation, so enabling
-- an unchanged OAuth account or switching its proxy incorrectly invalidated
-- capacity estimation for the rest of the official cycle. The new fingerprint
-- uses a stable Provider principal whenever one is available. Old hashes cannot
-- be converted without decoding OAuth material, while the whole-cycle estimator
-- can rebuild its state from RequestLog, so retain official usage and cold-start
-- only the derived estimator state.
UPDATE oauth_quota_snapshots
SET payload = CAST(
    json_set(
        CAST(payload AS TEXT),
        '$.estimator_state',
        json('null')
    )
    AS BLOB
)
WHERE json_type(CAST(payload AS TEXT), '$.estimator_state') <> 'null';
