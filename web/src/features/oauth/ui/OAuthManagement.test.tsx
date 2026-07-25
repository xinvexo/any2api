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

test("uses provider grid layout without a main-column session panel", async () => {
  mockAccounts([]);

  renderManagement();

  expect(await screen.findByRole("navigation", { name: "OAuth2 类型" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /Codex/ })).toHaveAttribute("aria-current", "page");
  expect(screen.getByRole("button", { name: /Grok/ })).toBeInTheDocument();
  expect(await screen.findByText("还没有 Codex OAuth 账号")).toBeInTheDocument();
  expect(screen.queryByText("还没有 Codex 登录会话")).not.toBeInTheDocument();
  expect(screen.queryByText(/配置版本/)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "OAuth认证" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "刷新" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "刷新全部额度" })).toBeDisabled();
  expect(screen.queryByLabelText("每页条数")).not.toBeInTheDocument();
  expect(screen.getByLabelText("账号数量")).toHaveTextContent("共 0 个账号");
});

test("opens OAuth auth in a right drawer", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const path = String(input);
      if (path === "/api/admin/oauth/accounts") {
        return jsonResponse({ config_revision: 1, items: [] });
      }
      if (path === "/api/admin/oauth/start" && init?.method === "POST") {
        return jsonResponse({
          flow: "authorization_code",
          provider: "codex",
          session_id: "session-1",
          authorization_url: "https://auth.example/authorize",
          redirect_uri: "http://localhost:1455/auth/callback",
          expires_in_seconds: 600,
        });
      }
      throw new Error(`unexpected request: ${path}`);
    }),
  );

  renderManagement();
  await screen.findByText("还没有 Codex OAuth 账号");
  fireEvent.click(screen.getByRole("button", { name: "OAuth认证" }));

  expect(await screen.findByRole("dialog", { name: "Codex OAuth 认证" })).toBeInTheDocument();
  expect(await screen.findByRole("link", { name: "打开授权页" })).toBeInTheDocument();
  expect(screen.queryByText("Codex 授权会话")).not.toBeInTheDocument();
  expect(screen.queryByText(/期望跳转/)).not.toBeInTheDocument();
});

test("starts the selected Grok OAuth flow", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path === "/api/admin/oauth/accounts") {
      return jsonResponse({ config_revision: 1, items: [] });
    }
    if (path === "/api/admin/oauth/start" && init?.method === "POST") {
      expect(JSON.parse(String(init.body))).toEqual({ provider: "grok" });
      return jsonResponse({
        flow: "device_code",
        provider: "grok",
        session_id: "grok-session",
        user_code: "ABCD-1234",
        verification_uri: "https://accounts.x.ai/oauth2/device",
        verification_uri_complete:
          "https://accounts.x.ai/oauth2/device?user_code=ABCD-1234",
        expires_in_seconds: 1800,
        poll_interval_seconds: 5,
      });
    }
    if (path === "/api/admin/oauth/device/poll" && init?.method === "POST") {
      expect(JSON.parse(String(init.body))).toEqual({ session_id: "grok-session" });
      return jsonResponse({ status: "pending", retry_after_seconds: 60 });
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderManagement(["/oauth?kind=grok"]);
  await screen.findByText("还没有 Grok OAuth 账号");
  fireEvent.click(screen.getByRole("button", { name: "OAuth认证" }));

  expect(await screen.findByRole("dialog", { name: "Grok OAuth 认证" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "打开验证页" })).toHaveAttribute(
    "href",
    "https://accounts.x.ai/oauth2/device?user_code=ABCD-1234",
  );
  expect(screen.getByLabelText("设备授权码")).toHaveTextContent("ABCD-1234");
  await waitFor(() =>
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/admin/oauth/device/poll",
      expect.objectContaining({ method: "POST" }),
    ),
  );
  expect(fetchMock).toHaveBeenCalledWith(
    "/api/admin/oauth/start",
    expect.objectContaining({ method: "POST" }),
  );
});

test("automatically activates a completed Grok device login", async () => {
  const pollGate = deferred<void>();
  let activated = false;
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const path = String(input);
      if (path === "/api/admin/oauth/accounts") {
        return jsonResponse({
          config_revision: activated ? 2 : 1,
          items: activated
            ? [oauthAccountJson("grok-1", "grok@example.com", "grok")]
            : [],
        });
      }
      if (path === "/api/admin/oauth/start" && init?.method === "POST") {
        return jsonResponse({
          flow: "device_code",
          provider: "grok",
          session_id: "grok-session",
          user_code: "ABCD-1234",
          verification_uri: "https://accounts.x.ai/oauth2/device",
          verification_uri_complete: null,
          expires_in_seconds: 1800,
          poll_interval_seconds: 5,
        });
      }
      if (path === "/api/admin/oauth/device/poll" && init?.method === "POST") {
        await pollGate.promise;
        activated = true;
        return jsonResponse({
          status: "complete",
          account: {
            provider: "grok",
            account_id: "grok-1",
            label: "grok@example.com",
            requests_per_minute: null,
            enabled: true,
            safe_account_email: "grok@example.com",
            expires_at: 1_900_000_000,
            selected_model_count: 7,
            config_version: 1,
            config_revision: 2,
          },
        });
      }
      throw new Error(`unexpected request: ${path}`);
    }),
  );

  renderManagement(["/oauth?kind=grok"]);
  await screen.findByText("还没有 Grok OAuth 账号");
  fireEvent.click(screen.getByRole("button", { name: "OAuth认证" }));
  expect(await screen.findByLabelText("设备授权码")).toHaveTextContent("ABCD-1234");
  pollGate.resolve(undefined);

  await waitFor(() =>
    expect(screen.queryByRole("dialog", { name: "Grok OAuth 认证" })).not.toBeInTheDocument(),
  );
  expect(await screen.findByText("grok@example.com")).toBeInTheDocument();
});

test("switches provider kind and keeps accounts in the content column", async () => {
  mockAccounts([
    {
      id: "a1",
      provider_kind: "codex",
      label: "Codex One",
      requests_per_minute: null,
      enabled: true,
      safe_account_email: null,
      expires_at: null,
      token_version: 1,
      account_generation: 1,
      config_version: 1,
      selected_model_count: 0,
      models: [],
      available_models: ["gpt-5.5"],
      plan_type: "free",
      usage: usage(),
    },
  ]);

  renderManagement();
  expect(await screen.findByText("Codex One")).toBeInTheDocument();
  expect(screen.getByText("free")).toBeInTheDocument();
  expect(screen.getByText("成功 2")).toBeInTheDocument();
  expect(screen.getByText("失败 1")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "刷新全部额度" })).toBeEnabled();

  fireEvent.click(screen.getByRole("button", { name: /Claude/ }));
  expect(screen.getByRole("button", { name: /Claude/ })).toHaveAttribute("aria-current", "page");
  expect(screen.queryByText("Codex One")).not.toBeInTheDocument();
  expect(screen.getByText("还没有 Claude OAuth 账号")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "刷新全部额度" })).not.toBeInTheDocument();
});

test("virtualizes the full collection and refreshes every Codex quota", async () => {
  const items = [
    ...Array.from({ length: 12 }, (_, index) =>
      oauthAccountJson(`a${index + 1}`, `Codex ${index + 1}`, "codex", index !== 11),
    ),
    oauthAccountJson("claude-1", "Claude One", "claude"),
  ];
  const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
    const path = String(input);
    if (path === "/api/admin/oauth/accounts") {
      return jsonResponse({ config_revision: 1, items });
    }
    const quotaPrefix = "/api/admin/oauth/accounts/";
    const accountId =
      path.startsWith(quotaPrefix) && path.endsWith("/quota")
        ? path.slice(quotaPrefix.length, -"/quota".length)
        : null;
    if (accountId === "a12") {
      return errorResponse("oauth_quota_upstream_failed", 502);
    }
    if (accountId?.match(/^a\d+$/)) {
      return jsonResponse(quota(Number(accountId.slice(1))));
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

  const quotaPaths = fetchMock.mock.calls
    .map(([input]) => String(input))
    .filter((path) => path.endsWith("/quota"));
  expect(new Set(quotaPaths)).toEqual(
    new Set(
      Array.from(
        { length: 12 },
        (_, index) => `/api/admin/oauth/accounts/a${index + 1}/quota`,
      ),
    ),
  );
  expect(quotaPaths.some((path) => path.includes("claude-1"))).toBe(false);
  expect(client.getQueryData(oauthQueryKeys.quota("a11"))).toBeDefined();
  expect(client.getQueryState(oauthQueryKeys.quota("a12"))?.status).toBe("error");
});

test("limits refresh-all concurrency and locks account actions", async () => {
  const items = Array.from({ length: 8 }, (_, index) =>
    oauthAccountJson(`a${index + 1}`, `Codex ${index + 1}`, "codex"),
  );
  const quotaGates: Array<ReturnType<typeof deferred<void>>> = [];
  let active = 0;
  let maxActive = 0;
  const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
    const path = String(input);
    if (path === "/api/admin/oauth/accounts") {
      return jsonResponse({ config_revision: 1, items });
    }
    if (path.endsWith("/quota")) {
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
  expect(
    screen.getAllByRole("button", { name: "刷新额度" }).every((button) =>
      button.hasAttribute("disabled"),
    ),
  ).toBe(true);

  quotaGates.slice(0, 6).forEach((gate) => gate.resolve(undefined));
  await waitFor(() => expect(quotaGates).toHaveLength(8));
  quotaGates.slice(6).forEach((gate) => gate.resolve(undefined));

  const notification = await screen.findByRole("status");
  expect(notification).toHaveTextContent("已刷新全部 8 个 Codex 账号额度。");
  expect(notification.className).toContain("notification-card");
  expect(maxActive).toBe(6);
  expect(screen.getByRole("button", { name: "删除 Codex 1" })).toBeEnabled();
});

function renderManagement(initialEntries: string[] = ["/oauth"]) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: 10_000 } },
  });
  return {
    client,
    ...render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries}>
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
  providerKind: "codex" | "claude" | "grok",
  enabled = true,
) {
  return {
    id,
    provider_kind: providerKind,
    label,
    requests_per_minute: null,
    enabled,
    safe_account_email: null,
    expires_at: null,
    token_version: 1,
    account_generation: 1,
    config_version: 1,
    selected_model_count: 0,
    models: [],
    available_models:
      providerKind === "codex"
        ? ["gpt-5.5"]
        : providerKind === "claude"
          ? ["claude-sonnet-4-5"]
          : ["grok-4.5"],
    plan_type: "free",
    usage: usage(),
  };
}

function quota(accountNumber: number) {
  return {
    fetched_at: 1_900_000_000 + accountNumber,
    rate_limit: {
      allowed: true,
      limit_reached: false,
      primary_window: {
        used_percent: accountNumber,
        limit_window_seconds: 18_000,
        reset_after_seconds: 300,
        reset_at: 1_900_000_300,
      },
      secondary_window: null,
    },
    reset_credits: { available_count: 1, expires_at: [] },
  };
}

function errorResponse(code: string, status: number) {
  return new Response(
    JSON.stringify({ error: { code, message: "quota request failed" } }),
    { status, headers: { "Content-Type": "application/json" } },
  );
}

function mockAccounts(items: unknown[]) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      if (String(input) === "/api/admin/oauth/accounts") {
        return jsonResponse({ config_revision: 1, items });
      }
      throw new Error(`unexpected request: ${String(input)}`);
    }),
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
