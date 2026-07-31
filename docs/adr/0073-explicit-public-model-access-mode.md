# ADR-0073: Explicit public model access mode

- Status: Accepted
- Date: 2026-07-31
- Decision maker: maintainer

## Context

ADR-0049 encoded both "allow every published model" and an empty exact allowlist as `[]`. The Web control therefore had to turn deselecting the final model back into "allow all", making it impossible for an administrator to publish no models. The two administrator intents must have distinct persisted values and must remain distinct after route pruning.

## Decision

- Replace the `models.allowed` `string_list` value with the dedicated `model_access` setting type.
- Encode allow-all as the explicit JSON string `"all"`. Encode an exact allowlist as a JSON array of public model names; `[]` therefore denies every model.
- Keep `"all"` as the compiled default so a new instance exposes all configured routes until the administrator chooses a restricted array.
- Validate, sort and deduplicate every array as `PublicModelName`. Wildcards, prefixes, Provider inference and sentinel entries inside the array remain unsupported.
- During configuration publication, intersect array values with the public routes materialized by the same transaction. Persist an empty intersection as `[]`; never promote it to `"all"`. The `"all"` mode has no names to prune.
- Compile the mode into the same `PublishedSnapshot` policy used by request planning and `GET /v1/models`. An empty array returns an empty catalog and rejects every model before affinity, RPM reservation or upstream I/O.
- The Web switch writes `"all"` only when the administrator explicitly enables "allow all". With the switch off, deselecting the final model keeps the exact-array mode and writes `[]`.

## Consequences

- Allow-all, allow-some and allow-none are three unambiguous states represented by one setting key.
- SQLite needs no schema migration because setting overrides already store typed JSON. Existing explicit array overrides retain their array shape; under the corrected contract an existing `[]` means allow-none.
- The management setting contract gains the `model_access` value type, whose value is either `"all"` or a string array.
- Route removal cannot accidentally reopen the remaining public model surface.

## Verification

- Domain tests cover the explicit default, normalized arrays and empty-array denial.
- Storage and Runtime tests prove pruning retains empty-array denial and allow-all remains explicit.
- HTTP contracts prove `[]` empties `GET /v1/models` and rejects model requests before upstream execution, while `"all"` restores the catalog.
- React tests prove deselecting the final model keeps the switch off, displays zero allowed models and saves `[]`.
