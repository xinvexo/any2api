import { afterEach, expect, test, vi } from "vitest";

import { getRequestLogs } from "./request-log-api";

afterEach(() => vi.restoreAllMocks());

test("serializes exact request log filters without client-controlled audit headers", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async () => listResponse());

  await getRequestLogs("r2.cursor", 50, {
    outcome: "failed",
    operation: "messages",
    publicModel: "claude-test",
    gatewayApiKeyId: "11111111-1111-4111-8111-111111111111",
    credentialId: "22222222-2222-4222-8222-222222222222",
  });
  await getRequestLogs();

  const firstHeaders = fetchMock.mock.calls[0]?.[1]?.headers as Record<string, string>;
  expect(fetchMock.mock.calls[0]?.[0]).toBe(
    "/api/admin/request-logs?page_size=50&cursor=r2.cursor&outcome=failed&operation=messages&public_model=claude-test&gateway_api_key_id=11111111-1111-4111-8111-111111111111&credential_id=22222222-2222-4222-8222-222222222222",
  );
  expect(firstHeaders["X-Any2API-Log-Refresh"]).toBeUndefined();
});

function listResponse() {
  return new Response(
    JSON.stringify({
      items: [],
      total: 0,
      page_size: 20,
      cursor: null,
      next_cursor: null,
      telemetry: { queued_records: 0, in_flight_records: 0, dropped_records: 0, persisted_records: 0 },
      filter_options: {
        public_models: [],
        gateway_api_keys: [],
        provider_credentials: [],
        oauth_accounts: [],
      },
    }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}
