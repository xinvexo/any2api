# ADR-0048: Keep disabled OAuth account tokens alive

- Status: Accepted
- Date: 2026-07-26
- Amends: ADR-0033's enabled-account-only refresh scan

## Context

`OAuthAccount.enabled` currently serves two unrelated purposes: it excludes an account from routing and also excludes it from scheduled token refresh. A temporarily disabled account can therefore lose a rotating refresh credential or remain expired until it is re-enabled. That makes administrative suspension accidentally change credential-lifecycle behavior.

An OAuth account stored in SQLite is still an intentionally retained account even while disabled. Disabling it should stop client traffic, not abandon its authentication state.

## Decision

- `OAuthAccount.enabled` controls routing eligibility only. Disabled accounts remain absent from model routing, data-plane attempts, affinity selection and routing RPM use.
- The process-level refresh worker scans enabled and disabled accounts. An account is scheduled only when its persisted expiry is inside the configured lead window and its Provider document contains a refresh token.
- Refresh keeps the existing DIRECT/global-proxy path, strict SSRF policy, per-account singleflight gate, token-version CAS and serialized SQLite/reconcile/snapshot publication.
- A successful refresh preserves `enabled`, selected models, optional RPM and other management metadata. It advances only the authentication versions and configuration revision required by the existing refresh contract.
- Disabling an account does not cancel an already selected refresh. A request-side 401 refresh that races with disablement may finish updating token material, but replanning still observes `enabled=false` and cannot route through that account.
- Deleting the OAuth account is the operation that ends token keepalive. Accounts without a refresh token, or without a persisted expiry that can enter the lead window, do not generate speculative refresh traffic.

## Consequences

- Temporarily disabled accounts remain ready for later re-enablement without sending model traffic while disabled.
- Administrators who want to stop all Provider communication for an account must delete it rather than disable it.
- Refresh failures remain fail-closed and redacted; this decision adds no filesystem copy, browser state, second scheduler or new user-configurable limit.
