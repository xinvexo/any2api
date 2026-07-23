import { expect, test } from "vitest";

import { parseProviderEndpointConfiguration } from "./provider-contracts";

test("parses direct and bridged provider endpoint configurations", () => {
  const parsed = parseProviderEndpointConfiguration({
    config_revision: 3,
    items: [
      endpoint(),
      endpoint({
        id: "83f81a8c-b1de-4a76-99ad-88548a47720d",
        upstream_protocol_dialect: "openai_chat_completions",
      }),
    ],
    protocol_options: protocolOptions(),
  });

  expect(parsed.configRevision).toBe(3);
  expect(parsed.items[0]?.upstreamProtocolDialect).toBeNull();
  expect(parsed.items[1]?.upstreamProtocolDialect).toBe(
    "openai_chat_completions",
  );
  expect(
    parseProviderEndpointConfiguration({
      config_revision: 4,
      items: [endpoint({ base_url: "http://127.0.0.1:8080/v1" })],
      protocol_options: protocolOptions(),
    }).items[0]?.baseUrl,
  ).toBe("http://127.0.0.1:8080/v1");
});

test("rejects unregistered protocol pairs and unsafe response URL components", () => {
  expect(() =>
    parseProviderEndpointConfiguration({
      config_revision: 1,
      items: [{ ...endpoint(), protocol_dialect: "anthropic_messages" }],
      protocol_options: protocolOptions(),
    }),
  ).toThrow("invalid provider endpoint response");

  expect(() =>
    parseProviderEndpointConfiguration({
      config_revision: 1,
      items: [{ ...endpoint(), base_url: "https://user:pass@example.com" }],
      protocol_options: protocolOptions(),
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
    upstream_protocol_dialect: null,
    enabled: true,
    config_version: 1,
    ...overrides,
  };
}

function protocolOptions() {
  return [
    {
      provider_kind: "codex",
      accepted_protocol: "openai_responses",
      upstream_protocols: [
        "openai_responses",
        "openai_chat_completions",
      ],
    },
    {
      provider_kind: "codex",
      accepted_protocol: "openai_chat_completions",
      upstream_protocols: ["openai_chat_completions"],
    },
  ];
}
