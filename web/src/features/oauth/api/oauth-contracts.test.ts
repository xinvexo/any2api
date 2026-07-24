import { describe, expect, it } from "vitest";

import {
  parseOAuthAccountConfiguration,
  parseOAuthActivationResult,
  parseOAuthStartResult,
} from "./oauth-contracts";

describe("parseOAuthStartResult", () => {
  it("parses a valid OAuth2 start response", () => {
    expect(
      parseOAuthStartResult({
        provider: "codex",
        session_id: "session",
        authorization_url: "https://auth.example.com/authorize",
        redirect_uri: "http://localhost:1455/auth/callback",
        expires_in_seconds: 600,
      }),
    ).toEqual({
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
        provider: "other",
        session_id: "session",
        authorization_url: "https://auth.example.com/authorize",
        redirect_uri: "file:///tmp/callback",
        expires_in_seconds: 600,
      }),
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
          max_concurrency: 2,
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
          usage: usage(),
        },
      ],
    });

    expect(parsed.configRevision).toBe(4);
    expect(parsed.items[0]).toMatchObject({
      providerKind: "codex",
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
            max_concurrency: 1,
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
        max_concurrency: 1,
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
      maxConcurrency: 1,
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
        max_concurrency: 0,
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
