import { expect, test } from "vitest";

import type { SettingsConfiguration } from "../api/settings-contracts";
import { selectNewestSettingsConfiguration } from "./settings-cache";

test("does not replace a newer settings revision with an older response", () => {
  const current = configuration(4);
  expect(selectNewestSettingsConfiguration(current, configuration(3))).toBe(current);
  expect(selectNewestSettingsConfiguration(current, configuration(5)).configRevision).toBe(5);
});

function configuration(configRevision: number): SettingsConfiguration {
  return { configRevision, items: [] };
}
