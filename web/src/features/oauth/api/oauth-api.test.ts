import { afterEach, expect, test, vi } from "vitest";

import { resetOAuthAccountQuota, startOAuthLogin } from "./oauth-api";

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

test("starts OAuth login with the manually selected proxy", async () => {
  let requestInit: RequestInit | undefined;
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    expect(String(input)).toContain("/admin/oauth/start");
    requestInit = init;
    return new Response(
      JSON.stringify({
        flow: "authorization_code",
        provider: "codex",
        session_id: "session",
        authorization_url: "https://auth.example.com/authorize",
        redirect_uri: "http://localhost:1455/auth/callback",
        expires_in_seconds: 600,
      }),
      { status: 200, headers: { "Content-Type": "application/json" } },
    );
  });
  vi.stubGlobal("fetch", fetchMock);

  await startOAuthLogin("codex", {
    mode: "profile",
    proxyProfileId: "proxy-1",
  });

  expect(JSON.parse(String(requestInit?.body))).toEqual({
    provider: "codex",
    proxy_selection: {
      mode: "profile",
      proxy_profile_id: "proxy-1",
    },
  });
});
