import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { OAuthManagement } from "./OAuthManagement";
import {
  clearNotifications,
  getNotifications,
  NotificationHost,
} from "@/shared/notifications";

afterEach(() => {
  clearNotifications();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("keeps one OAuth chrome while the account query leaves its loading state", async () => {
  const accounts = deferred<Response>();
  vi.stubGlobal("fetch", vi.fn(() => accounts.promise));

  renderManagement();
  expect(screen.getByText("正在读取 OAuth 账号")).toBeInTheDocument();
  const navigation = screen.getByRole("navigation", { name: "OAuth2 类型" });

  accounts.resolve(jsonResponse({ config_revision: 1, items: [] }));
  expect(await screen.findByText("还没有 Codex OAuth 账号")).toBeInTheDocument();
  expect(screen.getByRole("navigation", { name: "OAuth2 类型" })).toBe(navigation);
  expect(screen.getAllByRole("navigation", { name: "OAuth2 类型" })).toHaveLength(1);
});

test("uses provider grid layout without a main-column session panel", async () => {
  mockAccounts([]);

  renderManagement();

  expect(await screen.findByRole("navigation", { name: "OAuth2 类型" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /Codex/ })).toHaveAttribute("aria-current", "page");
  expect(screen.getByRole("button", { name: /Grok/ })).toBeInTheDocument();
  const emptyState = await screen.findByRole("status", { name: "暂无 Codex OAuth 账号" });
  expect(emptyState.closest(".grid")).toHaveClass(
    "h-full",
    "grid-rows-[auto_auto_auto_minmax(0,1fr)]",
    "sm:grid-rows-[auto_minmax(0,1fr)]",
  );
  expect(screen.getByText("还没有 Codex OAuth 账号")).toBeInTheDocument();
  expect(screen.queryByText("还没有 Codex 登录会话")).not.toBeInTheDocument();
  expect(screen.queryByText(/配置版本/)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "OAuth认证" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "导入 JSON" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "刷新" })).toBeInTheDocument();
  const refreshQuota = screen.getByRole("button", { name: "刷新全部额度" });
  expect(refreshQuota).toBeDisabled();
  expect(refreshQuota).toHaveTextContent("刷新额度");
  expect(getNotifications()).toHaveLength(0);

  fireEvent.click(screen.getByRole("button", { name: "刷新" }));
  expect(await screen.findByText("OAuth 账号已刷新")).toBeInTheDocument();
  const cleanupInvalid = screen.getByRole("button", { name: "删除失效账号" });
  expect(cleanupInvalid).toBeDisabled();
  expect(cleanupInvalid).toHaveTextContent("清理失效");
  expect(cleanupInvalid).toHaveAttribute("data-variant", "danger");
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
  expect(await screen.findByText("已激活 OAuth 账号「grok@example.com」")).toBeInTheDocument();
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
      bot_flagged: null,
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
  expect(screen.getByRole("button", { name: "刷新全部额度" })).toBeDisabled();
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
    bot_flagged: providerKind === "grok" ? false : null,
    usage: usage(),
  };
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
