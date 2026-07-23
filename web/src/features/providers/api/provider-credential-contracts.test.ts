import { expect, test } from "vitest";

import {
  parseProviderCredentialConfiguration,
  parseProviderCredentialTestResult,
  parseProviderOAuthExchangeResult,
  parseProviderOAuthStartResult,
} from "./provider-credential-contracts";

test("parses redacted credentials and rejects plaintext secret fields", () => {
  const parsed = parseProviderCredentialConfiguration(configuration());
  expect(parsed.items[0]).toMatchObject({
    credentialKind: "api_key",
    fingerprint: "v1:0123456789abcdef",
    secretTail: "test",
    maxConcurrency: 4,
    models: ["gpt-5.1-codex"],
  });

  expect(() =>
    parseProviderCredentialConfiguration(
      configuration({ api_key: "must-not-enter-the-cache" }),
    ),
  ).toThrow("invalid provider credential response");
});

test("rejects invalid concurrency and fingerprint versions", () => {
  expect(() =>
    parseProviderCredentialConfiguration(configuration({ max_concurrency: 0 })),
  ).toThrow("invalid provider credential response");
  expect(() =>
    parseProviderCredentialConfiguration(configuration({ fingerprint: "v2:0123456789abcdef" })),
  ).toThrow("invalid provider credential response");
});

test("parses OAuth credentials without exposing a secret tail", () => {
  const parsed = parseProviderCredentialConfiguration(
    configuration({ credential_kind: "oauth2", secret_tail: null }),
  );
  expect(parsed.items[0]).toMatchObject({
    credentialKind: "oauth2",
    secretTail: null,
  });
  expect(() =>
    parseProviderCredentialConfiguration(
      configuration({ credential_kind: "oauth2", secret_tail: "leak" }),
    ),
  ).toThrow("invalid provider credential response");
});

test("parses OAuth flow responses and rejects returned tokens", () => {
  expect(
    parseProviderOAuthStartResult({
      session_id: "session-id",
      authorization_url: "https://auth.example.com/authorize?state=opaque",
      redirect_uri: "http://localhost:1455/auth/callback",
      expires_in_seconds: 600,
    }),
  ).toMatchObject({ sessionId: "session-id", expiresInSeconds: 600 });
  expect(
    parseProviderOAuthExchangeResult({
      config_revision: 4,
      provider_endpoint_id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
      credential_id: "75072ca7-d922-428d-a4f8-86401567da32",
      provider_kind: "codex",
      account_id: "account-id",
      email: "owner@example.com",
      organization_id: null,
    }),
  ).toMatchObject({ providerKind: "codex", email: "owner@example.com" });
  expect(() =>
    parseProviderOAuthExchangeResult({
      config_revision: 4,
      provider_endpoint_id: "endpoint",
      credential_id: "credential",
      provider_kind: "codex",
      account_id: null,
      email: null,
      organization_id: null,
      access_token: "must-not-enter-the-cache",
    }),
  ).toThrow("invalid provider OAuth exchange response");
});

test("parses credential test outcomes and rejects inconsistent states", () => {
  expect(parseProviderCredentialTestResult(testResult())).toMatchObject({
    reachable: true,
    accepted: true,
    catalogValid: true,
    statusCode: 200,
    authErrorCleared: true,
    models: ["gpt-5.1-codex"],
  });
  expect(() =>
    parseProviderCredentialTestResult(testResult({
      accepted: false,
      auth_error_cleared: true,
    })),
  ).toThrow("invalid provider credential test response");
  expect(() =>
    parseProviderCredentialTestResult(testResult({
      reachable: false,
      accepted: false,
      status_code: null,
      error_stage: null,
      failure_scope: "proxy",
      auth_error_cleared: false,
    })),
  ).toThrow("invalid provider credential test response");
});

function configuration(overrides: Record<string, unknown> = {}) {
  return {
    config_revision: 3,
    provider_endpoint_id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
    items: [
      {
        id: "75072ca7-d922-428d-a4f8-86401567da32",
        provider_endpoint_id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
        label: "Primary",
        credential_kind: "api_key",
        fingerprint: "v1:0123456789abcdef",
        secret_tail: "test",
        proxy_profile_id: "00000000-0000-0000-0000-000000000000",
        max_concurrency: 4,
        enabled: true,
        secret_schema_version: 1,
        secret_version: 1,
        credential_generation: 1,
        config_version: 1,
        models: ["gpt-5.1-codex"],
        ...overrides,
      },
    ],
  };
}

function testResult(overrides: Record<string, unknown> = {}) {
  return {
    config_revision: 3,
    provider_endpoint_config_version: 1,
    credential_config_version: 1,
    credential_generation: 1,
    secret_version: 1,
    proxy_config_version: 1,
    credential_id: "75072ca7-d922-428d-a4f8-86401567da32",
    provider_endpoint_id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
    proxy_id: "00000000-0000-0000-0000-000000000000",
    reachable: true,
    accepted: true,
    catalog_valid: true,
    status_code: 200,
    latency_ms: 12,
    auth_error_cleared: true,
    error_stage: null,
    failure_scope: null,
    models: ["gpt-5.1-codex"],
    ...overrides,
  };
}
