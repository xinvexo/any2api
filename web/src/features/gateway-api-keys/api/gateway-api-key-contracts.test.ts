import { describe, expect, test } from "vitest";

import {
  parseGatewayApiKeyConfiguration,
  parseGatewayApiKeySecretReceipt,
} from "./gateway-api-key-contracts";

const token = `a2k_v1_${"a".repeat(43)}`;

const item = {
  id: "key-1",
  name: "Desktop",
  token,
  token_prefix: "a2k_v1_aaaaaaaa",
  token_version: 1,
  config_version: 1,
  enabled: true,
  revoked_at: null,
  created_at: "2026-07-19 10:00:00",
  last_used_at: null,
};

describe("gateway API Key contracts", () => {
  test("parses configuration with plaintext token on each item", () => {
    const configuration = parseGatewayApiKeyConfiguration({
      config_revision: 2,
      items: [item],
    });
    expect(configuration.items[0].name).toBe("Desktop");
    expect(configuration.items[0].token).toBe(token);
  });

  test("rejects invalid token formats on items", () => {
    expect(() =>
      parseGatewayApiKeyConfiguration({
        config_revision: 2,
        items: [{ ...item, token: "short" }],
      }),
    ).toThrow();
  });

  test("keeps create/rotate receipt token and item tokens", () => {
    const receipt = parseGatewayApiKeySecretReceipt({
      config_revision: 2,
      items: [item],
      token,
    });
    expect(receipt.token).toBe(token);
    expect(receipt.configuration.items[0].token).toBe(token);
    expect(() =>
      parseGatewayApiKeySecretReceipt({
        config_revision: 2,
        items: [item],
        token: "short",
      }),
    ).toThrow();
  });
});
