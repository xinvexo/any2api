import { describe, expect, test } from "vitest";

import {
  parseGatewayApiKeyConfiguration,
  parseGatewayApiKeySecretResponse,
} from "./gateway-api-key-contracts";

const token = `sk-${"a".repeat(43)}`;
const windowSlots = Array.from({ length: 30 }, (_, index) => ({
  started_at_ms: 1_720_000_000_000 + index * 120_000,
  total_requests: index === 28 ? 1 : index === 29 ? 2 : 0,
  successful_requests: index >= 28 ? 1 : 0,
  failed_requests: index === 29 ? 1 : 0,
}));

const item = {
  id: "key-1",
  name: "Desktop",
  token_prefix: token.slice(0, 16),
  token_version: 1,
  config_version: 1,
  requests_per_minute: null,
  enabled: true,
  created_at: "2026-07-19 10:00:00",
  last_used_at: null,
  usage: {
    total_requests: 3,
    successful_requests: 2,
    failed_requests: 1,
    window_minutes: 2,
    window_slots: windowSlots,
  },
};

describe("gateway API Key contracts", () => {
  test("parses secret-free configuration and usage statistics", () => {
    const configuration = parseGatewayApiKeyConfiguration({ config_revision: 2, items: [item] });
    expect(configuration.items[0].name).toBe("Desktop");
    expect(configuration.items[0]).not.toHaveProperty("token");
    expect(configuration.items[0].requestsPerMinute).toBeNull();
    expect(configuration.items[0].usage).toMatchObject({
      totalRequests: 3,
      successfulRequests: 2,
      failedRequests: 1,
      windowMinutes: 2,
    });
    expect(configuration.items[0].usage.windowSlots).toHaveLength(30);
    expect(configuration.items[0].usage.windowSlots[29]).toEqual({
      startedAtMs: windowSlots[29].started_at_ms,
      totalRequests: 2,
      successfulRequests: 1,
      failedRequests: 1,
    });
  });

  test("parses bounded RPM limits and rejects invalid values", () => {
    expect(
      parseGatewayApiKeyConfiguration({
        config_revision: 2,
        items: [{ ...item, requests_per_minute: 600 }],
      }).items[0].requestsPerMinute,
    ).toBe(600);
    for (const requests_per_minute of [0, 100_001]) {
      expect(() =>
        parseGatewayApiKeyConfiguration({
          config_revision: 2,
          items: [{ ...item, requests_per_minute }],
        }),
      ).toThrow();
    }
  });

  test("accepts a one-time mutation token without adding it to configuration items", () => {
    const result = parseGatewayApiKeySecretResponse({
      config_revision: 2,
      items: [item],
      token,
    });
    expect(result.token).toBe(token);
    expect(result.configuration.items[0]).not.toHaveProperty("token");
  });

  test("rejects plaintext tokens in ordinary items and invalid one-time tokens", () => {
    expect(() =>
      parseGatewayApiKeyConfiguration(
        {
          config_revision: 2,
          items: [{ ...item, token }],
        } as unknown as Parameters<typeof parseGatewayApiKeyConfiguration>[0],
      ),
    ).toThrow();
    expect(() =>
      parseGatewayApiKeyConfiguration({
        config_revision: 2,
        items: [{ ...item, token_prefix: token }],
      }),
    ).toThrow();
    expect(() =>
      parseGatewayApiKeySecretResponse({
        config_revision: 2,
        items: [item],
        token: "short",
      }),
    ).toThrow();
    expect(() =>
      parseGatewayApiKeySecretResponse({
        config_revision: 2,
        items: [item],
        token: `sk-${"b".repeat(43)}`,
      }),
    ).toThrow();
    expect(() =>
      parseGatewayApiKeyConfiguration({
        config_revision: 2,
        items: [
          {
            ...item,
            usage: {
              ...item.usage,
              window_slots: windowSlots.map((slot, index) =>
                index === 10 ? { ...slot, started_at_ms: slot.started_at_ms + 1 } : slot,
              ),
            },
          },
        ],
      }),
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

});
