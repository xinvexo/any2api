import { expect, test } from "vitest";

import { parseModelRouteConfiguration } from "./model-route-contracts";

test("parses an atomic model route aggregate", () => {
  const parsed = parseModelRouteConfiguration({
    config_revision: 4,
    items: [route()],
  });

  expect(parsed.configRevision).toBe(4);
  expect(parsed.items[0]?.fallbackOnSaturation).toBeNull();
  expect(parsed.items[0]?.targets[0]).toMatchObject({
    upstreamModel: "gpt-5.1-codex",
    fallbackTier: 0,
  });
});

test("rejects malformed model names, tiers and duplicate target identities", () => {
  expect(() =>
    parseModelRouteConfiguration({
      config_revision: 1,
      items: [{ ...route(), public_model: " codex-main" }],
    }),
  ).toThrow("invalid model route response");

  expect(() =>
    parseModelRouteConfiguration({
      config_revision: 1,
      items: [{ ...route(), targets: [{ ...target(), fallback_tier: 65_536 }] }],
    }),
  ).toThrow("invalid model route response");

  expect(() =>
    parseModelRouteConfiguration({
      config_revision: 1,
      items: [{ ...route(), targets: [target(), target()] }],
    }),
  ).toThrow("invalid model route response");
});

function route() {
  return {
    id: "f9937387-09ba-4d7a-ad08-2ab214aace86",
    public_model: "codex-main",
    ingress_protocol: "openai_responses",
    fallback_on_saturation: null,
    enabled: true,
    config_version: 1,
    targets: [target()],
  };
}

function target() {
  return {
    id: "f78b62bc-13e7-45ce-9df3-11a067160db7",
    provider_endpoint_id: "59b274af-f540-41d8-bf24-95ef07277502",
    upstream_model: "gpt-5.1-codex",
    fallback_tier: 0,
    enabled: true,
  };
}
