import { expect, test } from "vitest";

import type { OAuthAccount } from "../api/oauth-contracts";
import type { OAuthQuotaSnapshot } from "../api/oauth-quota-contracts";
import { presentOAuthAccount } from "./oauth-account-presentation";

test("real Credits keep a rolling-limit account visually available", () => {
  const available = presentOAuthAccount(account(), quota("rate_limit_reached"));
  const hardStopped = presentOAuthAccount(
    account(),
    quota("workspace_member_usage_limit_reached"),
  );

  expect(available.badges.map((badge) => badge.key)).not.toContain("quota-exhausted");
  expect(hardStopped.badges.map((badge) => badge.key)).toContain("quota-exhausted");
  expect(available.metrics.find((metric) => metric.key === "rpm")).toMatchObject({
    label: "60s RPM",
    value: "无限制",
  });
  expect(available.metrics.map((metric) => metric.key)).toEqual([
    "rpm",
    "in-flight",
    "models",
    "expires",
  ]);
  expect(available.metrics.find((metric) => metric.key === "expires"))
    .toMatchObject({ label: "过期", value: "未知" });
});

test("does not expose runtime status as a card metric", () => {
  const stopped = presentOAuthAccount({
    ...account(),
    runtime: { ...account().runtime, status: "endpoint_disabled" },
  });

  expect(stopped.metrics.some((metric) => metric.key === "runtime-status")).toBe(false);
});

test.each([
  ["ready", "正常", "success"],
  ["disabled", "停用", "warning"],
  ["endpoint_disabled", "停用", "warning"],
  ["authentication_expired", "过期", "danger"],
  ["rate_limited", "RPM 用尽", "warning"],
  ["proxy_disabled", "代理停用", "warning"],
] as const)("maps runtime status %s to the top badge", (status, label, tone) => {
  const presentation = presentOAuthAccount({
    ...account(),
    runtime: { ...account().runtime, status },
  });

  expect(presentation.badges).toContainEqual({
    key: "runtime-status",
    label,
    tone,
  });
});

test("prioritizes expiry and exhaustion over a stopped runtime", () => {
  const stopped = {
    ...account(),
    enabled: false,
    runtime: { ...account().runtime, status: "disabled" as const },
  };
  const expired = presentOAuthAccount(
    { ...stopped, expiresAt: 1_899_999_999 },
    quota("workspace_member_usage_limit_reached"),
    1_900_000_000,
  );
  const exhausted = presentOAuthAccount(
    stopped,
    quota("workspace_member_usage_limit_reached"),
    1_900_000_000,
  );

  expect(expired.badges.filter((badge) => badge.key !== "plan")).toEqual([{
    key: "runtime-status",
    label: "过期",
    tone: "danger",
  }]);
  expect(exhausted.badges.filter((badge) => badge.key !== "plan")).toEqual([{
    key: "quota-exhausted",
    label: "耗尽",
    tone: "warning",
  }]);
});

test("folds reauthorization into the prioritized expired status", () => {
  const presentation = presentOAuthAccount({
    ...account(),
    runtime: { ...account().runtime, status: "disabled" },
    tokenRefreshFailure: {
      tokenVersion: 1,
      trigger: "authentication_failure",
      stage: "token_endpoint",
      reason: "refresh_token_invalidated",
      upstreamStatus: 401,
      failureScope: null,
      occurredAt: 1_900_000_000,
      reauthorizationRequired: true,
    },
  });

  expect(presentation.badges.filter((badge) => badge.key !== "plan")).toEqual([{
    key: "token-refresh-failed",
    label: "过期",
    tone: "danger",
  }]);
});

function account(): OAuthAccount {
  return {
    id: "account-1",
    providerKind: "codex",
    label: "Primary Codex",
    requestsPerMinute: null,
    proxySelection: { mode: "global" },
    enabled: true,
    safeAccountEmail: null,
    expiresAt: null,
    tokenVersion: 1,
    accountGeneration: 1,
    configVersion: 1,
    selectedModelCount: 1,
    models: ["gpt-5.6-sol"],
    availableModels: ["gpt-5.6-sol"],
    runtime: {
      resolvedProxy: {
        id: "00000000-0000-0000-0000-000000000000",
        name: "DIRECT",
        kind: "direct",
        enabled: true,
      },
      rpm60s: { used: 0, limit: null },
      inFlight: 0,
      status: "ready",
    },
    planType: "plus",
    botFlagged: null,
    tokenRefreshFailure: null,
    usage: {
      totalRequests: 0,
      successfulRequests: 0,
      failedRequests: 0,
      windowMinutes: 2,
      windowSlots: [],
    },
  };
}

function quota(
  reachedType: NonNullable<OAuthQuotaSnapshot["access"]>["reachedType"],
): OAuthQuotaSnapshot {
  return {
    fetchedAt: 1_900_000_000,
    rateLimit: {
      allowed: false,
      limitReached: true,
      windows: [],
    },
    credits: {
      hasCredits: true,
      unlimited: false,
      balance: "17.50",
    },
    access: {
      spendControlReached: false,
      reachedType,
    },
    resetCredits: null,
    billing: null,
    tokenBalance: null,
    subscriptionTier: null,
    accountStatus: null,
    estimates: [],
    rateCard: {
      id: "openai_codex_credits_2026_08_11",
      creditsPerUsd: 25,
    },
  };
}
