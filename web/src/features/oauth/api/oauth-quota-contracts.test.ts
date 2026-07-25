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
          windows: [{
            id: "primary",
            kind: "time",
            used_percent: 37.5,
            limit_window_seconds: 18_000,
            reset_after_seconds: 300,
            reset_at: 1_900_000_300,
          }],
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
        windows: [{
          id: "primary",
          kind: "time",
          usedPercent: 37.5,
          limitWindowSeconds: 18_000,
          resetAfterSeconds: 300,
          resetAt: 1_900_000_300,
        }],
      },
      resetCredits: {
        availableCount: 2,
        expiresAt: ["2026-07-30T00:00:00Z"],
      },
    });
  });

  it("preserves an unknown Grok quota period without inventing reset data", () => {
    const parsed = parseOAuthQuotaSnapshot({
      fetched_at: 1_900_000_000,
        rate_limit: {
          allowed: true,
          limit_reached: false,
          windows: [{
            id: "requests",
            kind: "requests",
            used_percent: 25,
            limit_window_seconds: null,
            reset_after_seconds: null,
            reset_at: null,
          }],
      },
      reset_credits: null,
    });

    expect(parsed.rateLimit?.windows[0]).toEqual({
      id: "requests",
      kind: "requests",
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
      reset_credits: null,
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
