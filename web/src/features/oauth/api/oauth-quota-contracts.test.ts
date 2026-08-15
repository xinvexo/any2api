import { describe, expect, it } from "vitest";

import {
  parseNullableOAuthQuotaSnapshot,
  parseOAuthQuotaManualRefreshResult,
  parseOAuthQuotaResetResult,
  parseOAuthQuotaSnapshot,
} from "./oauth-quota-contracts";

describe("OAuth quota contracts", () => {
  it("accepts an account without a persisted quota snapshot", () => {
    expect(parseNullableOAuthQuotaSnapshot(null)).toBeNull();
    expect(() => parseNullableOAuthQuotaSnapshot(undefined)).toThrow(
      "invalid OAuth quota response",
    );
  });

  it("requires the model catalog result from a manual refresh", () => {
    const payload = {
      ...currentNullableSnapshot(),
      model_catalog_refreshed: true,
    };
    expect(parseOAuthQuotaManualRefreshResult(payload)).toEqual({
      snapshot: parseOAuthQuotaSnapshot(payload),
      modelCatalogRefreshed: true,
    });

    delete (payload as Record<string, unknown>).model_catalog_refreshed;
    expect(() => parseOAuthQuotaManualRefreshResult(payload)).toThrow(
      "invalid OAuth quota response",
    );
  });

  it("parses safe rate-limit windows and reset credits", () => {
    expect(
      parseOAuthQuotaSnapshot({
        fetched_at: 1_900_000_000,
        rate_limit: {
          allowed: true,
          limit_reached: false,
          windows: [{
            id: "primary",
            kind: "time",
            used_percent: 37.5,
            limit_window_seconds: 18_000,
            reset_after_seconds: 300,
            reset_at: 1_900_000_300,
          }],
        },
        credits: {
          has_credits: true,
          unlimited: false,
          balance: "17.50",
        },
        access: {
          spend_control_reached: false,
          reached_type: "rate_limit_reached",
        },
        reset_credits: {
          available_count: 2,
          expires_at: ["2026-07-30T00:00:00Z"],
        },
        billing: null,
        token_balance: null,
        subscription_tier: null,
        account_status: null,
        rate_card: {
          id: "openai_codex_credits_2026_08_11",
          credits_per_usd: 25,
        },
        estimates: [{
          window_id: "primary",
          window_kind: "time",
          limit_window_seconds: 18_000,
          window_reset_at: 1_900_000_300,
          estimated_capacity_credits: 25,
          estimated_used_credits: 9.375,
          estimated_remaining_credits: 15.625,
          completed_interval_count: 3,
        }],
      }),
    ).toEqual({
      fetchedAt: 1_900_000_000,
      rateLimit: {
        allowed: true,
        limitReached: false,
        windows: [{
          id: "primary",
          kind: "time",
          usedPercent: 37.5,
          limitWindowSeconds: 18_000,
          resetAfterSeconds: 300,
          resetAt: 1_900_000_300,
        }],
      },
      credits: {
        hasCredits: true,
        unlimited: false,
        balance: "17.50",
      },
      access: {
        spendControlReached: false,
        reachedType: "rate_limit_reached",
      },
      resetCredits: {
        availableCount: 2,
        expiresAt: ["2026-07-30T00:00:00Z"],
      },
      billing: null,
      tokenBalance: null,
      subscriptionTier: null,
      accountStatus: null,
      rateCard: {
        id: "openai_codex_credits_2026_08_11",
        creditsPerUsd: 25,
      },
      estimates: [{
        windowId: "primary",
        windowKind: "time",
        limitWindowSeconds: 18_000,
        windowResetAt: 1_900_000_300,
        estimatedCapacityCredits: 25,
        estimatedUsedCredits: 9.375,
        estimatedRemainingCredits: 15.625,
        completedIntervalCount: 3,
      }],
    });
  });

  it("parses Grok billing amounts and the current subscription tier", () => {
    const parsed = parseOAuthQuotaSnapshot({
      fetched_at: 1_900_000_000,
      rate_limit: null,
      credits: null,
      access: null,
      reset_credits: null,
      billing: {
        currency: "USD",
        prepaid_balance_minor: -2500,
        on_demand_used_minor: 125,
        on_demand_cap_minor: 5000,
        is_unified_billing_user: true,
      },
      token_balance: {
        source: "upstream",
        used: 250_000,
        limit: 2_000_000,
        remaining: 1_750_000,
        window_seconds: null,
      },
      subscription_tier: "SuperGrokPro",
      account_status: {
        authentication: "valid",
        user_blocked_reason: "BLOCKED_REASON_BILLING",
        team_blocked_reasons: ["BLOCKED_REASON_NO_LOGS"],
        quota_exhaustion: {
          observed_at: 1_900_000_000,
          used: 1_065_387,
          limit: 1_000_000,
        },
      },
      rate_card: null,
      estimates: [],
    });

    expect(parsed.billing).toEqual({
      currency: "USD",
      prepaidBalanceMinor: -2500,
      onDemandUsedMinor: 125,
      onDemandCapMinor: 5000,
      isUnifiedBillingUser: true,
    });
    expect(parsed.tokenBalance).toEqual({
      source: "upstream",
      used: 250_000,
      limit: 2_000_000,
      remaining: 1_750_000,
      windowSeconds: null,
    });
    expect(parsed.subscriptionTier).toBe("SuperGrokPro");
    expect(parsed.accountStatus).toEqual({
      authentication: "valid",
      userBlockedReason: "BLOCKED_REASON_BILLING",
      teamBlockedReasons: ["BLOCKED_REASON_NO_LOGS"],
      quotaExhaustion: {
        observedAt: 1_900_000_000,
        used: 1_065_387,
        limit: 1_000_000,
      },
    });
  });

  it("preserves a Grok credit window without inventing reset data", () => {
    const parsed = parseOAuthQuotaSnapshot({
      fetched_at: 1_900_000_000,
      rate_limit: {
          allowed: true,
          limit_reached: false,
          windows: [{
            id: "weekly_credits",
            kind: "credits",
            used_percent: 25,
            limit_window_seconds: null,
            reset_after_seconds: null,
            reset_at: null,
          }],
      },
      credits: null,
      access: null,
      reset_credits: null,
      billing: null,
      token_balance: null,
      subscription_tier: null,
      account_status: null,
      rate_card: null,
      estimates: [],
    });

    expect(parsed.rateLimit?.windows[0]).toEqual({
      id: "weekly_credits",
      kind: "credits",
      usedPercent: 25,
      limitWindowSeconds: null,
      resetAfterSeconds: null,
      resetAt: null,
    });
  });

  it("preserves every Claude window without inventing global availability", () => {
    const parsed = parseOAuthQuotaSnapshot({
      fetched_at: 1_900_000_000,
      rate_limit: {
        allowed: null,
        limit_reached: null,
        windows: [
          claudeWindow("five_hour", 12.5, 18_000),
          claudeWindow("seven_day", 34, 604_800),
          claudeWindow("seven_day_sonnet", 56, 604_800),
          claudeWindow("seven_day_overage_included", 78, 604_800),
        ],
      },
      credits: null,
      access: null,
      reset_credits: null,
      billing: null,
      token_balance: null,
      subscription_tier: null,
      account_status: null,
      rate_card: null,
      estimates: [],
    });

    expect(parsed.rateLimit?.allowed).toBeNull();
    expect(parsed.rateLimit?.limitReached).toBeNull();
    expect(parsed.rateLimit?.windows.map((window) => window.id)).toEqual([
      "five_hour",
      "seven_day",
      "seven_day_sonnet",
      "seven_day_overage_included",
    ]);
  });

  it("rejects unsafe numbers and malformed expiration lists", () => {
    expect(() =>
      parseOAuthQuotaSnapshot({
        fetched_at: 1,
        rate_limit: null,
        credits: null,
        access: null,
        reset_credits: { available_count: -1, expires_at: [] },
        billing: null,
        token_balance: null,
        subscription_tier: null,
        account_status: null,
        rate_card: null,
        estimates: [],
      }),
    ).toThrow("invalid OAuth quota response");
    expect(() =>
      parseOAuthQuotaSnapshot({
        fetched_at: 1,
        rate_limit: null,
        credits: null,
        access: null,
        reset_credits: null,
        billing: null,
        token_balance: {
          source: "local",
          used: 0,
          limit: 1_000_000,
          remaining: 1_000_000,
          window_seconds: 86_400,
        },
        subscription_tier: null,
        account_status: null,
        rate_card: null,
        estimates: [],
      }),
    ).toThrow("invalid OAuth quota response");
    expect(() =>
      parseOAuthQuotaSnapshot({
        fetched_at: 1,
        rate_limit: null,
        credits: null,
        access: null,
        reset_credits: { available_count: 1, expires_at: "secret" },
        billing: null,
        token_balance: null,
        subscription_tier: null,
        account_status: null,
        rate_card: null,
        estimates: [],
      }),
    ).toThrow("invalid OAuth quota response");
    expect(() =>
      parseOAuthQuotaSnapshot({
        fetched_at: 1,
        rate_limit: null,
        credits: null,
        access: null,
        reset_credits: null,
        billing: {
          currency: "USD",
          prepaid_balance_minor: Number.MAX_SAFE_INTEGER + 1,
          on_demand_used_minor: null,
          on_demand_cap_minor: null,
          is_unified_billing_user: true,
        },
        token_balance: null,
        subscription_tier: null,
        account_status: null,
        rate_card: null,
        estimates: [],
      }),
    ).toThrow("invalid OAuth quota response");
  });

  it("rejects omitted fields from the current nullable contract", () => {
    for (const field of [
      "billing",
      "token_balance",
      "subscription_tier",
      "account_status",
      "rate_card",
    ]) {
      const payload = currentNullableSnapshot();
      delete payload[field];
      expect(() => parseOAuthQuotaSnapshot(payload)).toThrow(
        "invalid OAuth quota response",
      );
    }

    const rateLimit = currentNullableSnapshot();
    rateLimit.rate_limit = { allowed: null, limit_reached: null, windows: [] };
    delete (rateLimit.rate_limit as Record<string, unknown>).allowed;
    expect(() => parseOAuthQuotaSnapshot(rateLimit)).toThrow(
      "invalid OAuth quota response",
    );
  });

  it("requires reset to confirm at least one window", () => {
    expect(parseOAuthQuotaResetResult({ windows_reset: 2 })).toEqual({
      windowsReset: 2,
    });
    expect(() => parseOAuthQuotaResetResult({ windows_reset: 0 })).toThrow(
      "invalid OAuth quota response",
    );
  });
});

function currentNullableSnapshot(): Record<string, unknown> {
  return {
    fetched_at: 1_900_000_000,
    rate_limit: null,
    credits: null,
    access: null,
    reset_credits: null,
    billing: null,
    token_balance: null,
    subscription_tier: null,
    account_status: null,
    rate_card: null,
    estimates: [],
  };
}

function claudeWindow(id: string, usedPercent: number, seconds: number) {
  return {
    id,
    kind: "time",
    used_percent: usedPercent,
    limit_window_seconds: seconds,
    reset_after_seconds: null,
    reset_at: 1_900_000_300,
  };
}
