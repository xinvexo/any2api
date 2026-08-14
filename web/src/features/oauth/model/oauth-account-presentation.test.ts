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
