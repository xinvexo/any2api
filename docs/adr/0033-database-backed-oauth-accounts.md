# ADR-0033: SQLite OAuthAccount 与统一路由

- Status: Accepted
- Date: 2026-07-24

## Context

OAuth accounts must remain separate from administrator-configured `ProviderEndpoint` and API-key-only `ProviderCredential` records. That management boundary must not create a second scheduler: Provider API Keys and OAuth accounts for the same protocol and model enter the same candidate selection, RPM, affinity, health, retry, and telemetry path.

SQLite is already the configuration truth, so storing OAuth material in additional files would introduce unnecessary dual-write and leakage risks.

## Decision

### Independent OAuthAccount persistence

`OAuthAccount` is a separate aggregate managed only through the OAuth page and OAuth admin API. It is not a `ProviderCredential` and is never nested beneath an administrator-configured Provider Endpoint.

SQLite stores the complete Provider OAuth JSON as plaintext together with stable ID, Provider, label, enabled state, fixed DIRECT proxy binding, optional `requests_per_minute`, selected models, token version, account generation, configuration version, safe account metadata, and timestamps. ADR-0074 applies the same plaintext persistence boundary to API Keys and proxy passwords.

SQLite is the only OAuth account truth source. A first interactive login and Provider-specific import create an account directly; an interactive login that uniquely matches the same stable Provider identity reauthorizes the existing account according to ADR-0087. The service does not return attachments, create server-local auth files, maintain a download cache, or expose a read/export endpoint. Generic Secret import/export remains prohibited.

Raw OAuth JSON and access/refresh/ID tokens never enter logs, management responses, React Query, browser storage, or Debug output. Account list responses expose only safe metadata such as ID, Provider, label, enabled state, model count, expiry, status, and redacted account identity.

### Separate management, common routing projection

`ProviderCredential` remains API-key-only and Provider management APIs do not gain an OAuth kind. At publication time, Provider credentials and OAuth accounts compile into a common internal `RoutingCredential` projection. The projection contains stable routing identity, Endpoint, actual proxy, model set, optional RPM, generation, and authentication material. The scheduler only consumes this projection, so both sources use the same atomic select-and-reserve operation, QueueTicket, fixed session affinity, health state, retry exclusions, response commit boundary, and runtime Guard lifetime.

OAuth routing IDs occupy a disjoint namespace from configured Credential IDs. Disabling an OAuth account removes it from route candidates but does not stop token keepalive; existing bindings remain fixed and fail locally while unavailable. Deleting the account retires the routing credential and clears its affinity. Re-enabling the same account preserves its stable routing identity and allows still-live in-memory bindings to resume.

OAuth accounts use Provider-owned fixed routing profiles rather than administrator-created Provider Endpoints:

- Codex uses the ChatGPT Codex Responses endpoint and OpenAI Responses dialect.
- Claude uses the official Anthropic API endpoint and Anthropic Messages dialect.
- Grok uses the xAI CLI subscription endpoint and its registered OpenAI Responses capability.
- Every OAuthAccount binds to `DIRECT`, inherits the configured global proxy, and has no hidden network fallback.
- Provider drivers own the OAuth model catalogs; updating a catalog is a local Provider change, not a scheduler branch.

These fixed profiles are internal routing projections only. They are never returned by, inserted into, or editable through Provider Endpoint APIs. New accounts default to no local RPM limit, the Provider's selected OAuth models, enabled state, and DIRECT/global-proxy routing. The OAuth page may edit label, enabled state, optional RPM, and selected models; it does not expose Endpoint or API-key Credential forms.

### Atomic activation and refresh

OAuth exchange consumes its one-time session before network I/O. After token exchange, Runtime constructs and validates a complete OAuthAccount candidate or same-identity reauthorization and executes the serialized publication flow:

```text
SQLite transaction writes OAuthAccount metadata + Provider JSON
-> validate and compile the complete configuration and routing projection
-> commit SQLite
-> reconcile Runtime state
-> one ArcSwap<PublishedSnapshot>
-> return safe JSON success
```

Success responses use `Cache-Control: no-store` and contain only safe account metadata plus the new PublishedSnapshot revision. They never return token fields or trigger a browser download.

Same-identity interactive reauthorization preserves the stable local account ID and administrator-owned label, RPM, enabled state and still-valid model selection. It replaces Token material through the current token-version CAS and never expands selected models implicitly. Stable Provider account ID takes precedence over email; email is only a fallback when both compared Token documents lack an account ID. Ambiguous duplicate identities fail closed. Detailed matching and conflict semantics are defined by ADR-0087.

A single process worker scans all accounts approaching expiry, including `enabled=false` accounts. `oauth.refresh.scan_interval` and `oauth.refresh.lead_time` are hot-reload SettingRegistry values, and the lead time cannot be shorter than the scan interval. Startup and PublishedSnapshot revision changes wake the worker so it always rescans current accounts and settings.

Refresh uses the account's DIRECT/global-proxy path and a per-account singleflight gate. The gate reloads and compares token version after acquisition, so stale results cannot overwrite newer material. Scheduled network refreshes use a fixed code-level concurrency limit. Currently ready results are opportunistically grouped into segments of at most that concurrency limit; each segment is token-version-CAS applied in one SQLite transaction and one serialized snapshot publication without waiting for a slow account elsewhere in the scan. Stale or deleted accounts are skipped without blocking fresh results. Authentication-failure refresh remains an immediate single-account publication so a pending request does not wait for a scheduled segment.

Token endpoint encoding remains Provider-owned: Codex authorization-code exchange is form encoded while Codex refresh is JSON; Claude exchanges are JSON; Grok device and refresh exchanges are form encoded. A structured permanent refresh rejection is remembered in process for the rejected account token version, suppressing all later refresh submissions for that same version. Reauthorization or another successful token replacement advances the version and clears the suppression by construction; it is not persisted across process restart.

A refresh or verified same-identity reauthorization increments `token_version` without incrementing `account_generation`. Runtime creates fresh authentication health for the new Token while reusing account-level quota, permission, and model cooldown state. Re-enabling an account still increments `account_generation` and resets both health scopes. ADR-0095 defines the isolation proof for late failures from retired Token generations.

Refresh failure never falls back to another network path. A still-valid access token may remain eligible until expiry; an expired or authentication-rejected account is fail-closed. A 401-triggered refresh/retry is allowed at most once and only while the attempt is `Pending`, `RetrySafety` permits it, and no downstream headers or bytes have been committed.

## Consequences

- OAuth management and persistence stay independent from Provider management while routing has one candidate pool and one RPM/affinity implementation.
- Multiple OAuth accounts are ordinary SQLite rows; no filesystem watcher or SQLite/file dual write exists.
- OAuth tokens are plaintext in local SQLite by explicit product decision. Database file permissions and host access are the protection boundary for this data class.
- The Web flow activates an account record directly and manages its state, RPM and models on the same page.
