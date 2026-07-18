import { expect, test } from "vitest";

import { selectNewestGatewayApiKeyConfiguration } from "./gateway-api-key-cache";

test("gateway API Key cache never moves backwards in revision", () => {
  const current = { configRevision: 4, items: [] };
  expect(selectNewestGatewayApiKeyConfiguration(current, { configRevision: 3, items: [] })).toBe(
    current,
  );
  expect(selectNewestGatewayApiKeyConfiguration(current, { configRevision: 5, items: [] })).toEqual({
    configRevision: 5,
    items: [],
  });
});
