import { describe, expect, it } from "vitest";

import {
  parseOAuthAccountConfiguration,
  parseOAuthActivationResult,
  parseOAuthDevicePollResult,
  parseOAuthImportResult,
  parseOAuthStartResult,
} from "./oauth-contracts";

describe("parseOAuthImportResult", () => {
  it("parses only safe imported account metadata", () => {
    const parsed = parseOAuthImportResult({
      imported_count: 1,
      config_revision: 2,
      items: [
        {
          id: "account-1",
          provider_kind: "grok",
          label: "Grok Imported",
          requests_per_minute: null,
          enabled: true,
          safe_account_email: "grok@example.com",
          expires_at: 1_900_000_000,
          selected_model_count: 7,
          config_version: 1,
        },
      ],
    });

    expect(parsed).toEqual({
      importedCount: 1,
      configRevision: 2,
      items: [
        {
          id: "account-1",
          providerKind: "grok",
          label: "Grok Imported",
          requestsPerMinute: null,
          enabled: true,
          safeAccountEmail: "grok@example.com",
          expiresAt: 1_900_000_000,
          selectedModelCount: 7,
          configVersion: 1,
        },
      ],
    });
    expect(JSON.stringify(parsed)).not.toContain("token");
  });

  it("rejects a count mismatch", () => {
    expect(() =>
      parseOAuthImportResult({ imported_count: 2, config_revision: 2, items: [] }),
    ).toThrow("invalid OAuth2 login response");
  });
});

describe("parseOAuthStartResult", () => {
  it("parses a valid OAuth2 start response", () => {
    expect(
      parseOAuthStartResult({
        flow: "authorization_code",
        provider: "codex",
        session_id: "session",
        authorization_url: "https://auth.example.com/authorize",
        redirect_uri: "http://localhost:1455/auth/callback",
        expires_in_seconds: 600,
      }),
    ).toEqual({
      flow: "authorization_code",
      provider: "codex",
      sessionId: "session",
      authorizationUrl: "https://auth.example.com/authorize",
      redirectUri: "http://localhost:1455/auth/callback",
      expiresInSeconds: 600,
    });
  });

  it("rejects an invalid provider or redirect URI", () => {
    expect(() =>
      parseOAuthStartResult({
        flow: "authorization_code",
        provider: "other",
        session_id: "session",
        authorization_url: "https://auth.example.com/authorize",
        redirect_uri: "file:///tmp/callback",
        expires_in_seconds: 600,
      }),
    ).toThrow("invalid OAuth2 login response");
  });

  it("parses the Grok device-code login contract", () => {
    expect(
      parseOAuthStartResult({
        flow: "device_code",
        provider: "grok",
        session_id: "grok-session",
        user_code: "ABCD-1234",
        verification_uri: "https://accounts.x.ai/oauth2/device",
        verification_uri_complete:
          "https://accounts.x.ai/oauth2/device?user_code=ABCD-1234",
        expires_in_seconds: 1800,
        poll_interval_seconds: 5,
      }),
    ).toEqual({
      flow: "device_code",
      provider: "grok",
      sessionId: "grok-session",
      userCode: "ABCD-1234",
      verificationUri: "https://accounts.x.ai/oauth2/device",
      verificationUriComplete:
        "https://accounts.x.ai/oauth2/device?user_code=ABCD-1234",
      expiresInSeconds: 1800,
      pollIntervalSeconds: 5,
    });
  });
});

describe("parseOAuthDevicePollResult", () => {
  it("parses pending and completed polls", () => {
    expect(
      parseOAuthDevicePollResult({ status: "pending", retry_after_seconds: 5 }),
    ).toEqual({ status: "pending", retryAfterSeconds: 5 });
    expect(
      parseOAuthDevicePollResult({
        status: "complete",
        account: {
          provider: "grok",
          account_id: "account-id",
          label: "grok@example.com",
          requests_per_minute: null,
          enabled: true,
          safe_account_email: "grok@example.com",
          expires_at: 1_900_000_000,
          selected_model_count: 7,
          config_version: 1,
          config_revision: 2,
        },
      }),
    ).toMatchObject({
      status: "complete",
      account: { provider: "grok", selectedModelCount: 7 },
    });
  });

  it("rejects an invalid retry interval", () => {
    expect(() =>
      parseOAuthDevicePollResult({ status: "pending", retry_after_seconds: 0 }),
    ).toThrow("invalid OAuth2 login response");
  });
});

describe("parseOAuthAccountConfiguration", () => {
  it("parses safe account metadata and selected models", () => {
    const parsed = parseOAuthAccountConfiguration({
      config_revision: 4,
      items: [
        {
          id: "fdcb6e74-820f-4d84-9df6-38af2b031feb",
          provider_kind: "codex",
          label: "Primary Codex",
          requests_per_minute: 2,
          enabled: true,
          safe_account_email: "person@example.com",
          expires_at: 1_800_000_000,
          token_version: 2,
          account_generation: 3,
          config_version: 4,
          selected_model_count: 2,
          models: ["gpt-5.5", "gpt-5.6-luna"],
          available_models: [
            "codex-auto-review",
            "gpt-5.4-mini",
            "gpt-5.5",
            "gpt-5.6-luna",
            "gpt-5.6-terra",
          ],
          plan_type: "plus",
          bot_flagged: null,
          token_refresh_failure: {
            token_version: 2,
            trigger: "scheduled",
            stage: "token_endpoint",
            reason: "refresh_token_reused",
            upstream_status: 400,
            failure_scope: "egress_path",
            occurred_at: 1_800_000_100,
            reauthorization_required: true,
          },
          usage: usage(),
        },
      ],
    });

    expect(parsed.configRevision).toBe(4);
    expect(parsed.items[0]).toMatchObject({
      providerKind: "codex",
      requestsPerMinute: 2,
      tokenVersion: 2,
      models: ["gpt-5.5", "gpt-5.6-luna"],
      availableModels: [
        "codex-auto-review",
        "gpt-5.4-mini",
        "gpt-5.5",
        "gpt-5.6-luna",
        "gpt-5.6-terra",
      ],
      planType: "plus",
      botFlagged: null,
      tokenRefreshFailure: {
        tokenVersion: 2,
        stage: "token_endpoint",
        reason: "refresh_token_reused",
        upstreamStatus: 400,
        failureScope: "egress_path",
        reauthorizationRequired: true,
      },
      usage: {
        totalRequests: 3,
        successfulRequests: 2,
        failedRequests: 1,
      },
    });
    expect(JSON.stringify(parsed)).not.toContain("access_token");
  });

  it("rejects a model count mismatch", () => {
    expect(() =>
      parseOAuthAccountConfiguration({
        config_revision: 1,
        items: [
          {
            id: "account",
            provider_kind: "claude",
            label: "Claude",
            requests_per_minute: null,
            enabled: true,
            safe_account_email: null,
            expires_at: null,
            token_version: 1,
            account_generation: 1,
            config_version: 1,
            selected_model_count: 2,
            models: ["claude-sonnet-4-6"],
            available_models: ["claude-sonnet-4-6"],
            plan_type: null,
            bot_flagged: null,
            token_refresh_failure: null,
            usage: usage(),
          },
        ],
      }),
    ).toThrow("invalid OAuth2 login response");
  });
});

function usage() {
  const windowMs = 2 * 60 * 1000;
  const newest = Math.floor(Date.now() / windowMs) * windowMs;
  return {
    total_requests: 3,
    successful_requests: 2,
    failed_requests: 1,
    window_minutes: 2,
    window_slots: Array.from({ length: 30 }, (_, index) => ({
      started_at_ms: newest - (29 - index) * windowMs,
      total_requests: index >= 27 ? 1 : 0,
      successful_requests: index === 27 || index === 29 ? 1 : 0,
      failed_requests: index === 28 ? 1 : 0,
    })),
  };
}

describe("parseOAuthActivationResult", () => {
  it("parses safe activated account metadata", () => {
    expect(
      parseOAuthActivationResult({
        provider: "claude",
        account_id: "fdcb6e74-820f-4d84-9df6-38af2b031feb",
        label: "person@example.com",
        requests_per_minute: null,
        enabled: true,
        safe_account_email: "person@example.com",
        expires_at: 1_800_000_000,
        selected_model_count: 3,
        config_version: 1,
        config_revision: 2,
      }),
    ).toEqual({
      provider: "claude",
      accountId: "fdcb6e74-820f-4d84-9df6-38af2b031feb",
      label: "person@example.com",
      requestsPerMinute: null,
      enabled: true,
      safeAccountEmail: "person@example.com",
      expiresAt: 1_800_000_000,
      selectedModelCount: 3,
      configVersion: 1,
      configRevision: 2,
    });
  });

  it("rejects malformed activation metadata", () => {
    expect(() =>
      parseOAuthActivationResult({
        provider: "codex",
        account_id: "account",
        label: "Codex",
        requests_per_minute: 0,
        enabled: true,
        safe_account_email: null,
        expires_at: null,
        selected_model_count: 0,
        config_version: 1,
        config_revision: 2,
      }),
    ).toThrow("invalid OAuth2 login response");
  });
});
