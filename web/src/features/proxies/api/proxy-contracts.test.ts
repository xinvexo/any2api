import { expect, test } from "vitest";

import { parseProxyTestResult } from "./proxy-contracts";

test("parses an egress-path-scoped proxy probe failure", () => {
  expect(
    parseProxyTestResult({
      config_revision: 7,
      proxy_config_version: 3,
      proxy_id: "proxy-1",
      reachable: false,
      status_code: null,
      latency_ms: 29,
      error_stage: "await_headers",
      failure_scope: "egress_path",
    }),
  ).toMatchObject({
    reachable: false,
    failureScope: "egress_path",
  });
});
