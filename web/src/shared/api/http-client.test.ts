import { afterEach, expect, test, vi } from "vitest";

import {
  ADMIN_SESSION_EXPIRED_EVENT,
  requestJson,
  setAdminCsrfToken,
} from "./http-client";

afterEach(() => {
  vi.unstubAllGlobals();
  setAdminCsrfToken(null);
});

test("adds the in-memory CSRF token only to administrator mutations", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    void input;
    void init;
    return new Response(JSON.stringify({ ok: true }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  });
  vi.stubGlobal("fetch", fetchMock);
  setAdminCsrfToken("csrf-token");

  await requestJson("/api/admin/settings/example", { method: "PATCH", body: {} });
  await requestJson("/api/admin/settings");

  const mutationHeaders = fetchMock.mock.calls[0]?.[1]?.headers as Record<string, string>;
  const queryHeaders = fetchMock.mock.calls[1]?.[1]?.headers as Record<string, string>;
  expect(mutationHeaders["X-CSRF-Token"]).toBe("csrf-token");
  expect(queryHeaders["X-CSRF-Token"]).toBeUndefined();
});

test("expires a protected administrator session before reading a 401 body", async () => {
  let resolveJson: (value: unknown) => void = () => undefined;
  const json = new Promise<unknown>((resolve) => {
    resolveJson = resolve;
  });
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({
      ok: false,
      status: 401,
      json: () => json,
    }) as Response),
  );
  let expired = false;
  const handleExpired = () => {
    expired = true;
  };
  window.addEventListener(ADMIN_SESSION_EXPIRED_EVENT, handleExpired);

  const pending = requestJson("/api/admin/settings").catch(() => undefined);
  await vi.waitFor(() => expect(expired).toBe(true));
  resolveJson({
    error: { code: "admin_session_required", message: "session required" },
  });
  await pending;
  window.removeEventListener(ADMIN_SESSION_EXPIRED_EVENT, handleExpired);
});
