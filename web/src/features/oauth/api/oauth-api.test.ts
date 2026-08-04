import { afterEach, expect, test, vi } from "vitest";

import { resetOAuthAccountQuota } from "./oauth-api";

afterEach(() => {
  vi.unstubAllGlobals();
});

test("reuses the quota reset request id after an ambiguous client failure", async () => {
  const bodies: string[] = [];
  const fetchMock = vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
    bodies.push(String(init?.body));
    if (bodies.length === 1) {
      return new Response(
        JSON.stringify({
          error: { code: "oauth_quota_upstream_failed", message: "try again" },
        }),
        { status: 502, headers: { "Content-Type": "application/json" } },
      );
    }
    return new Response(JSON.stringify({ windows_reset: 1 }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  });
  vi.stubGlobal("fetch", fetchMock);

  await expect(resetOAuthAccountQuota("retry-account")).rejects.toThrow("try again");
  await expect(resetOAuthAccountQuota("retry-account")).resolves.toEqual({
    windowsReset: 1,
  });

  const requestIds = bodies.map(
    (body) => JSON.parse(body).redeem_request_id as string,
  );
  expect(requestIds[0]).toMatch(/^[0-9a-f-]{36}$/);
  expect(requestIds[1]).toBe(requestIds[0]);
});
