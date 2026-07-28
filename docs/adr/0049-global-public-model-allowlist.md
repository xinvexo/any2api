# ADR-0049: Global public model allowlist

> ADR-0058 对空列表语义的修订已合并到本文。

- Status: Accepted
- Date: 2026-07-26
- Decision maker: maintainer

## Context

Provider API Key credentials and OAuth accounts already own their individual upstream model selections. Those selections determine which routes can exist, but they do not provide a concise instance-wide switch for limiting what clients may request. Editing every account would duplicate policy, scale poorly, and conflate upstream capability with the public surface of the gateway.

The global policy must hot reload atomically with gateway authentication and routing. It must also avoid a second model configuration aggregate or a new publication path. SQLite setting overrides already use typed JSON and the SettingRegistry already provides default, override, effective and reset semantics.

## Decision

- Add the hot-reload SettingRegistry key `models.allowed` with value type `optional_string_list` and compiled default `null`.
- Compile the value into `ModelAccessPolicy::All | Only(BTreeSet<PublicModelName>)`. `null` means `All`; every array means `Only`, and `[]` therefore denies every public model.
- Every value is validated as `PublicModelName`, then sorted and deduplicated before persistence and publication. Wildcards, prefixes and Provider inference are not supported.
- Every configuration publication intersects an `Only` set with the public routes materialized by that same transaction. If deleting or changing the last API Key/OAuth account removes a model's final route, the publication also removes that name from the persisted setting override before commit. An empty intersection remains `Only(empty)` and must not reopen access.
- The policy is compiled into `PublishedSnapshot`. All generation and count-token endpoints check it during request planning, before affinity mutation, candidate selection, RPM reservation or upstream I/O.
- Unknown and explicitly disallowed models share the existing protocol-compatible model-not-found boundary. This does not disclose whether a hidden route exists.
- `GET /v1/models` returns the intersection of the published model catalog and the effective allowlist, with the existing stable sorting and deduplication.
- The authenticated settings response supplies the current unfiltered published model names as editor options for `models.allowed`. Options are presentation metadata, not persisted configuration.
- The allowlist is instance-wide. A GatewayApiKey cannot select or override it, and it does not mutate per-ProviderCredential or per-OAuthAccount model selections.

## Consequences

- Administrators can reduce the public model surface from one compact setting without touching any upstream account.
- Resetting the override restores the compiled `null` default and therefore allows every currently published model.
- Stale selections cannot survive removal of their final route. If pruning removes the final selected name, the normalized empty array continues to deny every model until the administrator changes or resets the policy.
- A settings update, model directory response and new inference request observe one coherent PublishedSnapshot revision.
- Setting values are no longer universally scalar or `Copy`; list values require owned cloning at configuration boundaries.
- SQLite needs no schema migration because setting overrides are already stored as JSON.

## Verification

- Domain tests cover `null`/array parsing, validation, sorting, deduplication and the `All` default.
- Storage tests cover JSON-array persistence, reset and fail-closed loading of invalid rows.
- Runtime tests prove `null` allows all, empty arrays deny all, non-empty arrays filter the catalog, and disallowed models fail before RPM reservation or transport execution.
- HTTP contracts cover OpenAI and Anthropic error envelopes, including `/v1/messages/count_tokens`.
- React tests cover list drafts, search, select/clear-visible behavior, save and reset-to-default.
