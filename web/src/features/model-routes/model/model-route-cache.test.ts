import { expect, test } from "vitest";

import type { ModelRouteConfiguration } from "../api/model-route-contracts";
import { selectNewestModelRouteConfiguration } from "./model-route-cache";

test("does not replace a newer model route cache revision", () => {
  const current = configuration(5);
  expect(selectNewestModelRouteConfiguration(current, configuration(4))).toBe(current);
  expect(selectNewestModelRouteConfiguration(current, configuration(6)).configRevision).toBe(6);
});

function configuration(configRevision: number): ModelRouteConfiguration {
  return { configRevision, items: [] };
}
