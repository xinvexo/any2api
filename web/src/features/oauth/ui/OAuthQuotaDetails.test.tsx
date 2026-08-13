import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type { OAuthQuotaSnapshot } from "../api/oauth-quota-contracts";
import { OAuthQuotaDetails } from "./OAuthQuotaDetails";

test("shows real Credits in dollars and the estimate beside its percentage", () => {
  render(
    <OAuthQuotaDetails
      quota={quota}
      provider="codex"
      showResetCredits={false}
    />,
  );

  expect(screen.getByText("$9.9371")).toHaveAttribute(
    "title",
    "248.4272780000 Credits · 25 Credits = $1 · openai_codex_credits_2026_08_11",
  );
  const estimate = screen.getByText("$0.38/$1.00");
  expect(estimate.parentElement).toContainElement(screen.getByText("63%"));
});

const quota: OAuthQuotaSnapshot = {
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
    balance: "248.4272780000",
  },
  access: null,
  resetCredits: null,
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
};
