import { describe, expect, test } from "vitest";

import { parseGatewayApiKeyConfiguration } from "./gateway-api-key-contracts";

const token = `a2k_v1_${"a".repeat(43)}`;
const windowSlots = Array.from({ length: 30 }, (_, index) => ({
  started_at_ms: 1_720_000_000_000 + index * 120_000,
  total_requests: index === 28 ? 1 : index === 29 ? 2 : 0,
  successful_requests: index >= 28 ? 1 : 0,
  failed_requests: index === 29 ? 1 : 0,
}));

const item = {
  id: "key-1",
  name: "Desktop",
  token,
  token_prefix: token.slice(0, 16),
  token_version: 1,
  config_version: 1,
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
  test("parses plaintext configuration and usage statistics", () => {
    const configuration = parseGatewayApiKeyConfiguration({ config_revision: 2, items: [item] });
    expect(configuration.items[0].name).toBe("Desktop");
    expect(configuration.items[0].token).toBe(token);
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

  test("rejects invalid token formats on items", () => {
    expect(() =>
      parseGatewayApiKeyConfiguration({
        config_revision: 2,
        items: [{ ...item, token: "short" }],
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
        items: [{ ...item, token: `sk-${"a".repeat(48)}` }],
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
