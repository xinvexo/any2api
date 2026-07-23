import { describe, expect, test } from "vitest";

import {
  parseGatewayApiKeyConfiguration,
  parseGatewayApiKeySecretReceipt,
} from "./gateway-api-key-contracts";

const item = {
  id: "key-1",
  name: "Desktop",
  token_prefix: "a2k_v1_abcdefghij",
  token_version: 1,
  config_version: 1,
  enabled: true,
  revoked_at: null,
  created_at: "2026-07-19 10:00:00",
  last_used_at: null,
  usage: {
    total_requests: 3,
    successful_requests: 2,
    failed_requests: 1,
    recent_outcomes: [{ status_code: 200 }, { status_code: 429 }, { status_code: 204 }],
  },
};

describe("gateway API Key contracts", () => {
  test("parses redacted configuration and rejects secret fields", () => {
    expect(parseGatewayApiKeyConfiguration({ config_revision: 2, items: [item] }).items[0].name).toBe(
      "Desktop",
    );
    expect(
      parseGatewayApiKeyConfiguration({ config_revision: 2, items: [item] }).items[0].usage,
    ).toEqual({
      totalRequests: 3,
      successfulRequests: 2,
      failedRequests: 1,
      recentOutcomes: [{ statusCode: 200 }, { statusCode: 429 }, { statusCode: 204 }],
    });
    expect(() =>
      parseGatewayApiKeyConfiguration({ config_revision: 2, items: [{ ...item, token: "secret" }] }),
    ).toThrow();
    expect(() =>
      parseGatewayApiKeyConfiguration({
        config_revision: 2,
        items: [
          {
            ...item,
            usage: { ...item.usage, failed_requests: 2 },
          },
        ],
      }),
    ).toThrow();
  });

  test("keeps the one-time token outside the normal configuration parser", () => {
    const token = `a2k_v1_${"a".repeat(43)}`;
    const receipt = parseGatewayApiKeySecretReceipt({ config_revision: 2, items: [item], token });
    expect(receipt.token).toBe(token);
    expect(receipt.configuration.items).toHaveLength(1);
    expect(() =>
      parseGatewayApiKeySecretReceipt({ config_revision: 2, items: [item], token: "short" }),
    ).toThrow();
  });
});
