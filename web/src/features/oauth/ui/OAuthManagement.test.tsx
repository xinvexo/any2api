import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { OAuthManagement } from "./OAuthManagement";

afterEach(() => vi.restoreAllMocks());

test("uses provider grid layout without a main-column session panel", async () => {
  mockAccounts([]);

  renderManagement();

  expect(await screen.findByRole("navigation", { name: "OAuth2 类型" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /Codex/ })).toHaveAttribute("aria-current", "page");
  expect(await screen.findByText("还没有 Codex OAuth 账号")).toBeInTheDocument();
  expect(screen.queryByText("还没有 Codex 登录会话")).not.toBeInTheDocument();
  expect(screen.queryByText(/配置版本/)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "OAuth认证" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "刷新" })).toBeInTheDocument();
  // Empty state keeps a single footer pagination control.
  expect(screen.getByLabelText("每页条数")).toBeInTheDocument();
  expect(screen.getByText("共 0 条")).toBeInTheDocument();
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

test("switches provider kind and keeps accounts in the content column", async () => {
  mockAccounts([
    {
      id: "a1",
      provider_kind: "codex",
      label: "Codex One",
      max_concurrency: 1,
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

  fireEvent.click(screen.getByRole("button", { name: /Claude/ }));
  expect(screen.getByRole("button", { name: /Claude/ })).toHaveAttribute("aria-current", "page");
  expect(screen.queryByText("Codex One")).not.toBeInTheDocument();
  expect(screen.getByText("还没有 Claude OAuth 账号")).toBeInTheDocument();
});

test("paginates accounts and changes page size from the toolbar", async () => {
  mockAccounts(
    Array.from({ length: 12 }, (_, index) => ({
      id: `a${index + 1}`,
      provider_kind: "codex",
      label: `Codex ${index + 1}`,
      max_concurrency: 1,
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
    })),
  );

  renderManagement();
  expect(await screen.findByText("Codex 1")).toBeInTheDocument();
  expect(screen.getByText("Codex 10")).toBeInTheDocument();
  expect(screen.queryByText("Codex 11")).not.toBeInTheDocument();
  expect(screen.getByText("共 12 条")).toBeInTheDocument();
  expect(screen.getByText("1/2")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  expect(screen.getByText("Codex 11")).toBeInTheDocument();
  expect(screen.getByText("Codex 12")).toBeInTheDocument();
  expect(screen.queryByText("Codex 1")).not.toBeInTheDocument();
  expect(screen.getByText("2/2")).toBeInTheDocument();

  fireEvent.change(screen.getByLabelText("每页条数"), { target: { value: "20" } });
  expect(screen.getByText("Codex 1")).toBeInTheDocument();
  expect(screen.getByText("Codex 12")).toBeInTheDocument();
  expect(screen.getByText("1/1")).toBeInTheDocument();
});

function renderManagement(initialEntries: string[] = ["/oauth"]) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries}>
        <OAuthManagement />
      </MemoryRouter>
    </QueryClientProvider>,
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
