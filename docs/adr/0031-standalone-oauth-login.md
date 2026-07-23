# ADR-0031: Standalone OAuth2 login and one-time JSON delivery

- Status: Accepted
- Date: 2026-07-24

## Context

Codex and Claude interactive OAuth2 login produces an authentication JSON file used by compatible external tools. This is different from an any2api `ProviderCredential`: it does not configure an upstream endpoint, participate in scheduling, or need proxy, concurrency, model, health, refresh, or configuration-generation state.

The previous design modeled OAuth2 as a second `ProviderCredential` kind and consequently added SQLite/Vault storage, refresh workers, Provider-page controls, and configuration publication behavior. That model made a one-shot login utility part of the data plane and violated the product boundary requested for the first implementation.

## Decision

OAuth2 login is a standalone admin tool with two protected endpoints:

- `POST /api/admin/oauth/start` accepts only `codex` or `claude` and returns an in-memory session ID, the Provider authorization URL, the fixed localhost redirect URI, and the expiry duration.
- `POST /api/admin/oauth/exchange` accepts that session ID and the complete callback URL, exchanges the authorization code, and immediately returns an attachment named `codex-auth.json` or `claude-auth.json`.

Provider drivers own only Provider-specific OAuth protocol details: fixed endpoints, client IDs, redirect URIs, authorization URL construction, token request construction, token response parsing, and final file schema serialization. Runtime owns session/PKCE state and network execution. Server owns authenticated DTOs and download headers. Web owns only local interaction state.

Sessions are memory-only, one-time, expire after ten minutes, and are limited to 64 concurrent entries. Callback parsing requires the exact configured scheme, authority, port, and path and verifies state in constant time. Token exchange always uses the built-in DIRECT transport, disables redirects through the existing transport boundary, limits the response to 64 KiB, and applies a 30-second read timeout.

The successful response uses `Content-Disposition: attachment`, `application/json`, `nosniff`, and the admin-wide `Cache-Control: no-store` policy. Token material and the generated file are never written to SQLite, Secret Vault, logs, `PublishedSnapshot`, React Query, browser storage, or a server-side download cache.

`ProviderCredential` remains API-key-only. Provider pages do not expose OAuth controls or OAuth credential kinds. No refresh worker is created.

## Consequences

- The login tool remains independent from Endpoint, proxy, concurrency, model discovery, health, and routing concerns.
- A failed exchange consumes the one-time session; the administrator starts a new login instead of replaying a code or callback.
- Process restart discards all active sessions without recovery.
- The generated JSON is compatible with the top-level Codex/Claude authentication-file fields used by CLIProxyAPI and accepted by the reviewed import paths in new-api and Sub2API.
- Future import of Provider OAuth2 JSON, OAuth-backed request execution, or `/backend-api/codex/responses` compatibility requires a separate ADR and explicit credential/data-plane semantics.
