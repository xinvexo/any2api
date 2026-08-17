import { expect, test } from "vitest";

import {
  attemptResultLabel,
  operationLabel,
  protocolLabel,
  resultBadgeLabel,
  resultTone,
  shouldShowAttemptTimeline,
  upstreamCredentialDisplay,
  upstreamSource,
} from "./request-log-presentation";

test("labels OpenAI Images request logs", () => {
  expect(protocolLabel("openai_images")).toBe("Images");
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
