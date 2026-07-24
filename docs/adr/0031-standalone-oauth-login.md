# ADR-0031: Standalone OAuth2 login and one-time JSON delivery

- Status: Superseded in part by ADR-0033
- Date: 2026-07-24

## Historical context

The initial Codex and Claude interactive OAuth2 login produced an authentication JSON file for compatible external tools. It intentionally did not create an any2api `ProviderCredential`.

The previous design modeled OAuth2 as a second `ProviderCredential` kind and consequently added SQLite/Vault storage, refresh workers, Provider-page controls, and configuration publication behavior. That model violated the required separation between Provider management and OAuth account management.

## Superseded decision

OAuth2 login was introduced as a standalone admin tool with two protected endpoints:

- `POST /api/admin/oauth/start` accepts only `codex` or `claude` and returns an in-memory session ID, the Provider authorization URL, the fixed localhost redirect URI, and the expiry duration.
- `POST /api/admin/oauth/exchange` accepted that session ID and the complete callback URL, exchanged the authorization code, and immediately returned an attachment named `codex-auth.json` or `claude-auth.json`.

Provider drivers own Provider-specific OAuth protocol details: fixed endpoints, client IDs, redirect URIs, authorization URL construction, token request construction, token response parsing, and file schema serialization. Runtime owns session/PKCE state and network execution. Server owns authenticated DTOs. Web owns only local interaction state.

Sessions are memory-only, one-time, expire after ten minutes, and are limited to 64 concurrent entries. Callback parsing requires the exact configured scheme, authority, port, and path and verifies state in constant time. Token exchange always uses the built-in DIRECT transport, disables redirects through the existing transport boundary, limits the response to 64 KiB, and applies a 30-second read timeout.

The successful response uses `Content-Disposition: attachment`, `application/json`, `nosniff`, and the admin-wide `Cache-Control: no-store` policy. Token material and the generated file are never written to SQLite, Secret Vault, logs, `PublishedSnapshot`, React Query, browser storage, or a server-side download cache.

`ProviderCredential` remains API-key-only. Provider pages do not expose OAuth controls or OAuth credential kinds.

## Consequences

- ADR-0033 replaces the browser-download and data-plane-isolation portions of this decision. OAuth remains a separate managed resource but its valid account files now compile into the common routing candidate pool.
- A failed exchange consumes the one-time session; the administrator starts a new login instead of replaying a code or callback.
- Process restart discards all active sessions without recovery.
- The generated JSON is compatible with the top-level Codex/Claude authentication-file fields used by CLIProxyAPI and accepted by the reviewed import paths in new-api and Sub2API.
- The previously published OAuth credential migrations remain immutable migration history. A forward migration restores the API-key-only database constraint and fails closed if a database still contains an OAuth credential, so an upgrade never silently deletes stored token material.
- The API-key-only `ProviderCredential` boundary remains in force; OAuth account files are not Provider Credential imports.
