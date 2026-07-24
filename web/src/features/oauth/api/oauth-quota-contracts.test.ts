import { describe, expect, it } from "vitest";

import {
  parseOAuthQuotaResetResult,
  parseOAuthQuotaSnapshot,
} from "./oauth-quota-contracts";

describe("OAuth quota contracts", () => {
  it("parses safe rate-limit windows and reset credits", () => {
    expect(
      parseOAuthQuotaSnapshot({
        fetched_at: 1_900_000_000,
        rate_limit: {
          allowed: true,
          limit_reached: false,
          primary_window: {
            used_percent: 37.5,
            limit_window_seconds: 18_000,
            reset_after_seconds: 300,
            reset_at: 1_900_000_300,
          },
          secondary_window: null,
        },
        reset_credits: {
          available_count: 2,
          expires_at: ["2026-07-30T00:00:00Z"],
        },
      }),
    ).toEqual({
      fetchedAt: 1_900_000_000,
      rateLimit: {
        allowed: true,
        limitReached: false,
        primaryWindow: {
          usedPercent: 37.5,
          limitWindowSeconds: 18_000,
          resetAfterSeconds: 300,
          resetAt: 1_900_000_300,
        },
        secondaryWindow: null,
      },
      resetCredits: {
        availableCount: 2,
        expiresAt: ["2026-07-30T00:00:00Z"],
      },
    });
  });

  it("rejects unsafe numbers and malformed expiration lists", () => {
    expect(() =>
      parseOAuthQuotaSnapshot({
        fetched_at: 1,
        rate_limit: null,
        reset_credits: { available_count: -1, expires_at: [] },
      }),
    ).toThrow("invalid OAuth quota response");
    expect(() =>
      parseOAuthQuotaSnapshot({
        fetched_at: 1,
        rate_limit: null,
        reset_credits: { available_count: 1, expires_at: "secret" },
      }),
    ).toThrow("invalid OAuth quota response");
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
