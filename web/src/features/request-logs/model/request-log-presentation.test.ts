import { expect, test } from "vitest";

import {
  attemptResultLabel,
  formatLogListTime,
  operationLabel,
  presentRequestQuotaCost,
  resultBadgeLabel,
  resultTone,
  shouldShowAttemptTimeline,
  upstreamCredentialDisplay,
  upstreamSource,
} from "./request-log-presentation";

test("formats request-list time like the compact system log table", () => {
  const localTime = new Date(2026, 7, 20, 15, 54, 34).getTime();

  expect(formatLogListTime(localTime)).toBe("08/20 15:54:34");
});

test("presents quota cost in dollars with exact billing context", () => {
  const cost = presentRequestQuotaCost({
    unit: "codex_credits",
    amountNanos: "248427278000",
    rateCard: "codex-rate-2026-08",
    serviceTier: "fast",
    creditsPerUsd: 25,
  });

  expect(cost).toEqual({
    value: "$9.937091",
    detail: "本地估算 · 248.427278 Credits · 25 Credits = $1 · 费率卡 codex-rate-2026-08 · 快速档",
  });
});

test("preserves tiny, historical, zero, and unavailable quota costs", () => {
  expect(presentRequestQuotaCost({
    unit: "codex_credits",
    amountNanos: "1",
    rateCard: "current",
    serviceTier: "standard",
    creditsPerUsd: 25,
  })?.value).toBe("<$0.000001");
  expect(presentRequestQuotaCost({
    unit: "codex_credits",
    amountNanos: "9375000000",
    rateCard: "historical",
    serviceTier: "standard",
    creditsPerUsd: null,
  })?.value).toBe("9.375 Credits");
  expect(presentRequestQuotaCost({
    unit: "codex_credits",
    amountNanos: "9007199254740993",
    rateCard: "historical",
    serviceTier: "standard",
    creditsPerUsd: null,
  })?.value).toBe("9007199.254740993 Credits");
  expect(presentRequestQuotaCost({
    unit: "codex_credits",
    amountNanos: "0",
    rateCard: "current",
    serviceTier: "standard",
    creditsPerUsd: 25,
  })?.value).toBe("$0");
  expect(presentRequestQuotaCost(null)).toBeNull();
});

test("labels OpenAI Images request logs", () => {
  expect(operationLabel("images_generations")).toBe(
    "/v1/images/generations",
  );
  expect(operationLabel("images_edits")).toBe("/v1/images/edits");
});

test("prefixes an API key label with its provider endpoint name", () => {
  const source = upstreamSource({
    oauthAccountId: null,
    credentialId: "credential-1",
    providerEndpointName: "frapi",
    credentialLabel: "key",
  });

  expect(source.kind).toBe("api_key");
  expect(source.displayName).toBe("frapi-key");
});

test("keeps OAuth labels independent from provider endpoint names", () => {
  const source = upstreamSource({
    oauthAccountId: "oauth-1",
    credentialId: null,
    providerEndpointName: "frapi",
    oauthAccountLabel: "work-oauth",
  });

  expect(source.kind).toBe("oauth");
  expect(source.displayName).toBe("work-oauth");
});

test("keeps the credential fallback when an endpoint name is unavailable", () => {
  const source = upstreamSource({
    oauthAccountId: null,
    credentialId: "credential-1",
    providerEndpointName: null,
    credentialLabel: "key",
  });

  expect(source.displayName).toBe("key");
});

test("identifies API keys by endpoint without adding an OAuth endpoint placeholder", () => {
  expect(upstreamCredentialDisplay({
    oauthAccountId: null,
    credentialId: "credential-1",
    providerEndpointName: "Claude",
    credentialLabel: "key3",
  })).toEqual({ label: "上游凭据", value: "Claude · key3" });

  expect(upstreamCredentialDisplay({
    oauthAccountId: "oauth-1",
    credentialId: null,
    providerEndpointName: null,
    oauthAccountLabel: "work@example.com",
  })).toEqual({ label: "上游凭据", value: "OAuth · work@example.com" });
});

test("renders a failed 200 stream from its final outcome", () => {
  expect(resultBadgeLabel("failed", 200)).toBe("失败 200");
  expect(resultTone("failed", 200)).toContain("text-danger");
  expect(resultBadgeLabel("success", 200)).toBe("成功");
});

test("labels attempts only from their own HTTP status", () => {
  expect(attemptResultLabel({ outcome: "failed", statusCode: 429 })).toBe(
    "失败 · HTTP 429",
  );
  expect(attemptResultLabel({ outcome: "failed", statusCode: null })).toBe(
    "失败 · 未收到 HTTP 状态",
  );
  expect(attemptResultLabel({ outcome: "cancelled", statusCode: 499 })).toBe(
    "已取消",
  );
});

test("shows attempt flow for retries even when the final request succeeds", () => {
  expect(shouldShowAttemptTimeline("success", 2)).toBe(true);
  expect(shouldShowAttemptTimeline("success", 1)).toBe(false);
  expect(shouldShowAttemptTimeline("failed", 1)).toBe(true);
  expect(shouldShowAttemptTimeline("cancelled", 1)).toBe(true);
});
