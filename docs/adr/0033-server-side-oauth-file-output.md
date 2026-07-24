# ADR-0033: Database-backed OAuth2 accounts and unified routing

- Status: Accepted
- Date: 2026-07-24
- Supersedes: ADR-0031's browser-download and data-plane isolation decisions

## Context

The standalone OAuth2 page currently returns a Codex or Claude authentication JSON as a browser download. That leaves any2api unable to use the authenticated account. An intermediate design considered a configured server-local auth directory, but once any2api owns account scheduling and refresh, keeping both SQLite and a JSON file would create an unnecessary dual-write consistency problem.

OAuth accounts must remain a separate management concept from administrator-configured `ProviderEndpoint` and API-key-only `ProviderCredential`. Separation at that boundary must not create a second scheduler: Provider API Keys and OAuth accounts for the same protocol and model must enter the same candidate selection, capacity, affinity, health, retry, and telemetry path.

The reviewed projects provide complementary reference points:

- CLIProxyAPI proves that one OAuth document can represent one schedulable account and that refreshed tokens must replace the selected account's material, but its filesystem watcher is unnecessary when SQLite is already any2api's configuration truth.
- Sub2API models OAuth credentials as independent accounts and routes them with other accounts, but depends on PostgreSQL, Redis, and distributed refresh coordination that a single-node any2api does not need.
- new-api stores one Codex OAuth JSON on one ordinary channel and injects it only after selection. Its database-first lifecycle is useful, while its channel/multi-tenant model and weak refresh serialization are not.

## Decision

### Independent OAuthAccount persistence

Introduce a separate `OAuthAccount` aggregate and SQLite tables. It is not a `ProviderCredential`, is never nested beneath an administrator-configured Provider Endpoint, and is managed only through the OAuth page and OAuth admin API.

SQLite stores the complete Provider OAuth JSON as plaintext together with account metadata: stable ID, Provider, label, enabled state, DIRECT proxy binding, maximum concurrency, selected models, token version, account generation, configuration version, safe account metadata, and timestamps. OAuth JSON intentionally does not use the Secret Vault. API Keys, proxy passwords, and other existing Secret classes keep their current encryption rules.

SQLite is the only OAuth account truth source. OAuth login does not return an attachment, create a server-local JSON file, maintain a download cache, or add an output-directory setting. A future explicit Provider-specific import/export feature requires another ADR; generic Secret import/export remains prohibited.

Raw OAuth JSON and access/refresh/ID tokens never enter logs, management responses, React Query, browser storage, or Debug output. Normal account list responses expose only safe metadata such as ID, Provider, label, enabled state, model count, expiry, status, and redacted account identity.

### Separate management, common routing projection

`ProviderCredential` remains API-key-only and Provider management APIs/Web pages do not gain an OAuth kind. At publication time, configured Provider credentials and enabled OAuth accounts are compiled into a common internal `RoutingCredential` projection. The projection has a stable routing ID, Endpoint, actual proxy, model set, maximum concurrency, generation, and authentication material. The scheduler only consumes this projection, so both sources use the existing atomic select-and-acquire operation, load ratio, QueueTicket, hard/soft affinity, health state, retry exclusions, response commit boundary, and Permit lifetime.

OAuth routing IDs occupy a disjoint UUID namespace from configured Credential IDs. Disabling or deleting an OAuth account retires that routing credential and clears its affinity exactly like removing a configured Credential. Re-enabling the same account preserves its stable routing identity for new requests but does not reconstruct lost in-memory bindings.

OAuth accounts use Provider-owned fixed routing profiles instead of administrator-created Provider Endpoints:

- Codex uses the ChatGPT Codex Responses endpoint and the OpenAI Responses dialect.
- Claude uses the official Anthropic API endpoint and the Anthropic Messages dialect.
- Both bind to `DIRECT`, therefore inherit the configured global proxy; there is no hidden network fallback.
- Provider drivers own the OAuth model catalog. Codex selects the compact catalog for the plan claim decoded from its ID token; Claude uses its registered OAuth catalog. Updating those catalogs is a local Provider change, not a scheduler branch.

These fixed profiles are internal routing projections only. They are never returned by, inserted into, or editable through Provider Endpoint APIs.

New OAuth accounts default to maximum concurrency `1`, selected Provider OAuth models, enabled state, and DIRECT/global-proxy routing. The OAuth account page may edit label, enabled state, maximum concurrency, and selected models through OAuth-specific endpoints. It does not expose Endpoint or Credential forms.

### Atomic activation and refresh

OAuth exchange consumes its one-time session before network I/O. After token exchange, the runtime constructs and validates a complete OAuthAccount candidate and executes the existing serialized publication flow:

```text
SQLite transaction writes OAuthAccount metadata + Provider JSON
→ validate and compile the complete configuration and routing projection
→ commit SQLite
→ reconcile Runtime state
→ one ArcSwap<PublishedSnapshot>
→ return safe JSON success
```

`POST /api/admin/oauth/exchange` returns `Cache-Control: no-store` JSON containing only Provider, OAuth account ID, safe account metadata, selected model count, and the new PublishedSnapshot revision. It never returns token fields or triggers a browser download.

A single process-level worker scans enabled accounts approaching expiry. `oauth.refresh.scan_interval` defaults to 30 seconds and `oauth.refresh.lead_time` defaults to 300 seconds; both are hot-reload SettingRegistry values and the lead time must not be shorter than the scan interval. Startup and PublishedSnapshot revision changes wake the worker so it always rescans the current account set and current effective settings.

Refresh uses the account's actual DIRECT resolution, including the global proxy, and a per-account singleflight gate. The gate reloads and compares token version after acquisition, so a stale refresh result cannot overwrite a newer login. Provider parsing preserves refresh responses' omitted stable refresh token, ID token, account identity, email, and the prior expiry as a fail-closed fallback. Successful refresh uses token-version CAS, preserves account metadata and selected models, creates a new routing generation, and completes one serialized snapshot publication.

Refresh failure never falls back to another network path. A still-valid access token may remain eligible until its actual expiry; an expired or authentication-rejected account is fail-closed and the unified scheduler may select another OAuth account or Provider API Key under the normal retry rules. A 401-triggered refresh/retry is allowed at most once and only while the attempt is still `Pending`, `RetrySafety` permits it, and no downstream headers or bytes have been committed.

The token exchange and refresh requests use the account's actual DIRECT/global-proxy path. A proxy failure is fail-closed and does not retry through local direct networking.

## Consequences

- OAuth management and persistence stay independent from Provider management, while routing has one candidate pool and one concurrency/affinity implementation.
- Multiple OAuth accounts are ordinary SQLite rows; no fixed filename overwrite, watcher race, or filesystem/SQLite dual-write exists.
- OAuth tokens are plaintext in the local SQLite database by explicit product decision. Database file permissions and host access are the protection boundary for this data class.
- The OAuth Web flow becomes simpler: authorize, paste callback URL, receive an activated account record, then manage its state and models on the same page.
