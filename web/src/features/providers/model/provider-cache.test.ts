import { expect, test } from "vitest";

import type { ProviderEndpointConfiguration } from "../api/provider-contracts";
import { selectNewestProviderConfiguration } from "./provider-cache";

test("does not replace a newer provider cache revision", () => {
  const current = configuration(4);
  expect(selectNewestProviderConfiguration(current, configuration(3))).toBe(current);
  expect(selectNewestProviderConfiguration(current, configuration(5)).configRevision).toBe(5);
});

function configuration(configRevision: number): ProviderEndpointConfiguration {
  return { configRevision, items: [], protocolOptions: [] };
}
