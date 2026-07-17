import { expect, test } from "vitest";

import type { ProxyConfiguration } from "../api/proxy-contracts";
import { selectNewestProxyConfiguration } from "./proxy-cache";

test("does not replace a newer cache revision with an older mutation response", () => {
  const current = configuration(4);
  const incoming = configuration(3);

  expect(selectNewestProxyConfiguration(current, incoming)).toBe(current);
  expect(selectNewestProxyConfiguration(current, configuration(5)).configRevision).toBe(5);
});

function configuration(configRevision: number): ProxyConfiguration {
  return {
    configRevision,
    globalProxyId: "direct",
    items: [],
  };
}
