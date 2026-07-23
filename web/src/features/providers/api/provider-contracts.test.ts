import { expect, test } from "vitest";

import { parseProviderEndpointConfiguration } from "./provider-contracts";

test("parses a compatible provider endpoint configuration", () => {
  const parsed = parseProviderEndpointConfiguration({
    config_revision: 3,
    items: [endpoint()],
  });

  expect(parsed.configRevision).toBe(3);
  expect(parsed.items[0]?.protocolDialect).toBe("openai_responses");
  expect(
    parseProviderEndpointConfiguration({
      config_revision: 4,
      items: [endpoint({ base_url: "http://127.0.0.1:8080/v1" })],
    }).items[0]?.baseUrl,
  ).toBe("http://127.0.0.1:8080/v1");
});

test("rejects incompatible dialects and unsafe response URL components", () => {
  expect(() =>
    parseProviderEndpointConfiguration({
      config_revision: 1,
      items: [{ ...endpoint(), protocol_dialect: "anthropic_messages" }],
    }),
  ).toThrow("invalid provider endpoint response");

  expect(() =>
    parseProviderEndpointConfiguration({
      config_revision: 1,
      items: [{ ...endpoint(), base_url: "https://user:pass@example.com" }],
    }),
  ).toThrow("invalid provider endpoint response");
});

function endpoint(overrides: Record<string, unknown> = {}) {
  return {
    id: "b9bc39b0-d05b-4731-a072-d05e48fb8a4f",
    name: "Codex Primary",
    provider_kind: "codex",
    base_url: "https://api.example.com/v1",
    protocol_dialect: "openai_responses",
    enabled: true,
    config_version: 1,
    ...overrides,
  };
}
