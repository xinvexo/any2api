import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { oauthQueryKeys } from "../model/oauth-query-keys";
import { OAuthManagement } from "./OAuthManagement";
import { clearNotifications, NotificationHost } from "@/shared/notifications";

afterEach(() => {
  clearNotifications();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("virtualizes the full collection and refreshes every Codex quota", async () => {
  const items = [
    ...Array.from({ length: 12 }, (_, index) =>
      oauthAccountJson(`a${index + 1}`, `Codex ${index + 1}`, "codex", index !== 11),
    ),
    oauthAccountJson("claude-1", "Claude One", "claude"),
  ];
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path === "/api/admin/oauth/accounts") {
      return jsonResponse({ config_revision: 1, items });
    }
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    const quotaPrefix = "/api/admin/oauth/accounts/";
    if (path.startsWith(quotaPrefix) && path.endsWith("/quota") && init?.method === "GET") {
      return jsonResponse(null);
    }
    const refreshSuffix = "/quota/refresh";
    const accountId =
      path.startsWith(quotaPrefix) &&
      path.endsWith(refreshSuffix) &&
      init?.method === "POST"
        ? path.slice(quotaPrefix.length, -refreshSuffix.length)
        : null;
    if (accountId === "a12") {
      return errorResponse("oauth_quota_upstream_failed", 502);
    }
    if (accountId?.match(/^a\d+$/)) {
      return jsonResponse(quota(Number(accountId.slice(1)), accountId === "a1"));
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  const { client } = renderManagement();
  expect(await screen.findByText("Codex 1")).toBeInTheDocument();
  expect(screen.queryByText("Codex 12")).not.toBeInTheDocument();
  expect(screen.getByRole("list", { name: "Codex OAuth 账号列表" })).toBeInTheDocument();
  expect(screen.getByLabelText("账号数量")).toHaveTextContent("共 12 个账号");
  expect(screen.queryByLabelText("每页条数")).not.toBeInTheDocument();

  const refreshAll = screen.getByRole("button", { name: "刷新全部额度" });
  fireEvent.click(refreshAll);
  const notification = await screen.findByRole("alert");
  expect(notification).toHaveTextContent("已刷新 11 个 Codex 账号额度，1 个失败。");
  expect(notification.className).toContain("notification-card");
  await waitFor(() => expect(refreshAll).toBeEnabled());

  const refreshPaths = fetchMock.mock.calls
    .filter(([, init]) => init?.method === "POST")
    .map(([input]) => String(input))
    .filter((path) => path.endsWith("/quota/refresh"));
  expect(new Set(refreshPaths)).toEqual(
    new Set(
      Array.from(
        { length: 12 },
        (_, index) => `/api/admin/oauth/accounts/a${index + 1}/quota/refresh`,
      ),
    ),
  );
  expect(refreshPaths.some((path) => path.includes("claude-1"))).toBe(false);
  expect(client.getQueryData(oauthQueryKeys.quota("a11"))).toBeDefined();
  expect(client.getQueryData(oauthQueryKeys.quota("a12"))).toBeUndefined();
  expect(screen.getByText("耗尽")).toBeInTheDocument();
});

test("keeps reauthorization accounts compact and marks the whole card", async () => {
  const failedAccount = {
    ...oauthAccountJson("reauthorize-1", "Needs Authorization", "codex", false),
    token_refresh_failure: {
      token_version: 1,
      trigger: "authentication_failure",
      stage: "token_endpoint",
      reason: "refresh_token_invalidated",
      upstream_status: 401,
      failure_scope: null,
      occurred_at: 1_900_000_000,
      reauthorization_required: true,
    },
  };
  const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
    const path = String(input);
    if (path === "/api/admin/oauth/accounts") {
      return jsonResponse({ config_revision: 1, items: [failedAccount] });
    }
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderManagement();

  expect(await screen.findByText("Needs Authorization")).toBeInTheDocument();
  expect(screen.getByLabelText("账号状态：过期")).toBeInTheDocument();
  expect(screen.queryByText("需重新授权")).not.toBeInTheDocument();
  const notice = screen.getByRole("alert", { name: "Token 刷新失败" });
  expect(notice).not.toHaveClass("border-t", "border-danger/20", "rounded-lg", "bg-danger/5");
  expect(notice.querySelector("svg")).toBeNull();
  const card = notice.closest("[data-floating-bounds]");
  expect(card).toHaveClass(
    "border-danger/20",
    "bg-linear-to-b",
    "from-danger/10",
    "via-danger/[0.035]",
    "to-surface",
  );
  expect(screen.queryByRole("region", { name: "Codex 额度" })).not.toBeInTheDocument();
  expect(screen.queryByRole("group", { name: /Needs Authorization 近 1 小时/ }))
    .not.toBeInTheDocument();
  expect(fetchMock).toHaveBeenCalledTimes(2);
});

test("shows a permanent refresh diagnostic before the account refetch completes", async () => {
  const currentAccount = oauthAccountJson("a1", "Refresh Expired", "codex");
  const refreshFailure = {
    token_version: 1,
    trigger: "authentication_failure",
    stage: "token_endpoint",
    reason: "refresh_token_invalidated",
    upstream_status: 401,
    failure_scope: null,
    occurred_at: 1_900_000_000,
    reauthorization_required: true,
  };
  const accountRefetch = deferred<Response>();
  let accountReads = 0;
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path === "/api/admin/oauth/accounts") {
      accountReads += 1;
      if (accountReads === 1) {
        return jsonResponse({ config_revision: 1, items: [currentAccount] });
      }
      return accountRefetch.promise;
    }
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (path === "/api/admin/oauth/accounts/a1/quota" && init?.method !== "POST") {
      return jsonResponse(quota(1));
    }
    if (path === "/api/admin/oauth/accounts/a1/quota/refresh" && init?.method === "POST") {
      return oauthRefreshFailureResponse(refreshFailure);
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderManagement();
  expect(await screen.findByText("Refresh Expired")).toBeInTheDocument();
  fireEvent.click(await screen.findByRole("button", { name: "刷新额度" }));

  const notice = await screen.findByRole("alert", { name: "Token 刷新失败" });
  expect(notice).toHaveTextContent("Refresh Token 已被撤销");
  expect(screen.getByLabelText("账号状态：过期")).toBeInTheDocument();
  expect(screen.queryByText(/Refresh Endpoint 已明确拒绝/)).not.toBeInTheDocument();
  expect(screen.queryByRole("region", { name: "Codex 额度" })).not.toBeInTheDocument();
  await waitFor(() => expect(accountReads).toBe(2));

  accountRefetch.resolve(jsonResponse({
    config_revision: 1,
    items: [{ ...currentAccount, token_refresh_failure: refreshFailure }],
  }));
});

test("limits refresh-all concurrency and keeps card spinners stable", async () => {
  const items = Array.from({ length: 8 }, (_, index) =>
    oauthAccountJson(`a${index + 1}`, `Codex ${index + 1}`, "codex"),
  );
  const quotaGates: Array<ReturnType<typeof deferred<void>>> = [];
  let active = 0;
  let maxActive = 0;
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path === "/api/admin/oauth/accounts") {
      return jsonResponse({ config_revision: 1, items });
    }
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (path.endsWith("/quota") && init?.method === "GET") {
      return jsonResponse(null);
    }
    if (path.endsWith("/quota/refresh") && init?.method === "POST") {
      const gate = deferred<void>();
      quotaGates.push(gate);
      active += 1;
      maxActive = Math.max(maxActive, active);
      await gate.promise;
      active -= 1;
      return jsonResponse(quota(1));
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderManagement();
  expect(await screen.findByText("Codex 1")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "刷新全部额度" }));

  await waitFor(() => expect(quotaGates).toHaveLength(6));
  expect(maxActive).toBe(6);
  expect(screen.getByRole("button", { name: "删除 Codex 1" })).toBeDisabled();
  const quotaRefreshButtons = screen.getAllByRole("button", { name: "刷新额度" });
  expect(quotaRefreshButtons.every((button) => button.hasAttribute("disabled"))).toBe(true);
  expect(
    quotaRefreshButtons.every((button) =>
      button.querySelector("svg")?.classList.contains("animate-spin"),
    ),
  ).toBe(true);

  quotaGates.slice(0, 6).forEach((gate) => gate.resolve(undefined));
  await waitFor(() => expect(quotaGates).toHaveLength(8));
  expect(
    quotaRefreshButtons.every((button) =>
      button.querySelector("svg")?.classList.contains("animate-spin"),
    ),
  ).toBe(true);
  quotaGates.slice(6).forEach((gate) => gate.resolve(undefined));

  const notification = await screen.findByRole("status");
  expect(notification).toHaveTextContent("已刷新全部 8 个 Codex 账号额度。");
  expect(notification.className).toContain("notification-card");
  expect(maxActive).toBe(6);
  expect(screen.getByRole("button", { name: "删除 Codex 1" })).toBeEnabled();
  expect(
    screen.getAllByRole("button", { name: "刷新额度" }).every((button) =>
      !button.querySelector("svg")?.classList.contains("animate-spin"),
    ),
  ).toBe(true);
});

function renderManagement() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: 10_000 } },
  });
  return {
    client,
    ...render(
      <QueryClientProvider client={client}>
        <MemoryRouter initialEntries={["/oauth"]}>
          <OAuthManagement />
          <NotificationHost />
        </MemoryRouter>
      </QueryClientProvider>,
    ),
  };
}

function oauthAccountJson(
  id: string,
  label: string,
  providerKind: "codex" | "claude",
  enabled = true,
) {
  return {
    id,
    provider_kind: providerKind,
    label,
    requests_per_minute: null,
    proxy_selection: { mode: "global" },
    enabled,
    safe_account_email: null,
    expires_at: null,
    token_version: 1,
    account_generation: 1,
    config_version: 1,
    selected_model_count: 0,
    models: [],
    available_models:
      providerKind === "codex" ? ["gpt-5.5"] : ["claude-sonnet-4-5"],
    runtime: {
      resolved_proxy: {
        id: "00000000-0000-0000-0000-000000000000",
        name: "DIRECT",
        kind: "direct",
        enabled: true,
      },
      rpm_60s: { used: 0, limit: null },
      in_flight: 0,
      status: "ready",
    },
    plan_type: "free",
    bot_flagged: null,
    token_refresh_failure: null,
    usage: usage(),
  };
}

function proxyConfiguration() {
  return {
    config_revision: 1,
    global_proxy_id: "00000000-0000-0000-0000-000000000000",
    items: [
      {
        id: "00000000-0000-0000-0000-000000000000",
        name: "DIRECT",
        kind: "direct",
        host: null,
        port: null,
        username: null,
        password_configured: false,
        authentication_version: 0,
        enabled: true,
        built_in: true,
        config_version: 1,
      },
    ],
  };
}

function quota(accountNumber: number, exhausted = false) {
  return {
    fetched_at: 1_900_000_000 + accountNumber,
    rate_limit: {
      allowed: !exhausted,
      limit_reached: exhausted,
      windows: [{
        id: "primary",
        kind: "time",
        used_percent: exhausted ? 100 : accountNumber,
        limit_window_seconds: 18_000,
        reset_after_seconds: 300,
        reset_at: 1_900_000_300,
      }],
    },
    credits: null,
    access: null,
    reset_credits: { available_count: 1, expires_at: [] },
    billing: null,
    token_balance: null,
    subscription_tier: null,
    account_status: null,
    rate_card: {
      id: "openai_codex_credits_2026_08_11",
      credits_per_usd: 25,
    },
    estimates: [],
  };
}

function errorResponse(code: string, status: number) {
  return new Response(
    JSON.stringify({ error: { code, message: "quota request failed" } }),
    { status, headers: { "Content-Type": "application/json" } },
  );
}

function oauthRefreshFailureResponse(diagnostic: Record<string, unknown>) {
  return new Response(
    JSON.stringify({
      error: {
        code: "oauth_refresh_permanently_rejected",
        message: "the OAuth provider permanently rejected this account's refresh token",
        diagnostic,
      },
    }),
    { status: 502, headers: { "Content-Type": "application/json" } },
  );
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

function usage() {
  const windowMs = 2 * 60 * 1000;
  const newest = Math.floor(Date.now() / windowMs) * windowMs;
  return {
    total_requests: 3,
    successful_requests: 2,
    failed_requests: 1,
    window_minutes: 2,
    window_slots: Array.from({ length: 30 }, (_, index) => ({
      started_at_ms: newest - (29 - index) * windowMs,
      total_requests: index >= 27 ? 1 : 0,
      successful_requests: index === 27 || index === 29 ? 1 : 0,
      failed_requests: index === 28 ? 1 : 0,
    })),
  };
}
