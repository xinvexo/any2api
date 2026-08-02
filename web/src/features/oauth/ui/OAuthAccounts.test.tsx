import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import type { OAuthAccount } from "../api/oauth-contracts";
import { OAuthAccounts } from "./OAuthAccounts";
import { clearNotifications, getNotifications } from "@/shared/notifications";

afterEach(() => {
  clearNotifications();
  vi.restoreAllMocks();
});

test("lists and edits OAuth accounts without receiving token material", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path.endsWith(`/api/admin/oauth/accounts/${accountId}`) && init?.method === "PATCH") {
      expect(JSON.parse(String(init.body))).toEqual({
        expected_revision: 2,
        expected_config_version: 1,
        label: "Renamed Codex",
        requests_per_minute: 3,
        enabled: true,
      });
      expect(String(init.body)).not.toContain("token");
      return response({
        config_revision: 3,
        items: [accountJson("Renamed Codex", 2, 3)],
      });
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderAccounts([account("Primary Codex", 1)]);
  expect(screen.getByText("Primary Codex")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "编辑 Primary Codex" }));
  fireEvent.change(await screen.findByLabelText("账号名称"), {
    target: { value: "Renamed Codex" },
  });
  fireEvent.change(screen.getByLabelText("RPM 限制"), { target: { value: "3" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
  expect(getNotifications()).toEqual([
    expect.objectContaining({ message: "已保存「Renamed Codex」", tone: "success" }),
  ]);
});

test("selects and saves OAuth routing models through the account-specific endpoint", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (
      path.endsWith(`/api/admin/oauth/accounts/${accountId}/models`) &&
      init?.method === "PUT"
    ) {
      expect(JSON.parse(String(init.body))).toEqual({
        expected_revision: 2,
        expected_config_version: 1,
        models: ["gpt-5.5", "gpt-5.6-luna"],
      });
      expect(String(init.body)).not.toContain("token");
      return response({
        config_revision: 3,
        items: [
          accountJson("Primary Codex", 2, null, ["gpt-5.5", "gpt-5.6-luna"]),
        ],
      });
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderAccounts([account("Primary Codex", 1)]);
  fireEvent.click(screen.getByRole("button", { name: "查看 Primary Codex 的可用模型" }));
  expect(await screen.findByRole("checkbox", { name: "gpt-5.5" })).toBeChecked();
  fireEvent.click(screen.getByRole("checkbox", { name: "gpt-5.6-luna" }));
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
  expect(getNotifications()).toEqual([
    expect.objectContaining({ message: "已保存「Primary Codex」的模型选择", tone: "success" }),
  ]);
});

test("shows kind-scoped empty state without a session panel", () => {
  renderAccounts([]);
  const emptyState = screen.getByRole("status", { name: "暂无 Codex OAuth 账号" });
  expect(emptyState).toHaveClass("h-full", "min-h-40");
  expect(emptyState).not.toHaveClass("border", "bg-surface");
  expect(screen.getByText("还没有 Codex OAuth 账号")).toBeInTheDocument();
  expect(screen.queryByText(/点击「OAuth认证」/)).not.toBeInTheDocument();
  expect(screen.queryByText("还没有 Codex 登录会话")).not.toBeInTheDocument();
});

function renderAccounts(items: OAuthAccount[]) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/oauth"]}>
        <OAuthAccounts provider="codex" accounts={items} configRevision={2} />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const accountId = "fdcb6e74-820f-4d84-9df6-38af2b031feb";

function account(label: string, configVersion: number): OAuthAccount {
  return {
    id: accountId,
    providerKind: "codex",
    label,
    requestsPerMinute: null,
    enabled: true,
    safeAccountEmail: "person@example.com",
    expiresAt: 1_900_000_000,
    tokenVersion: 1,
    accountGeneration: 1,
    configVersion,
    selectedModelCount: 1,
    models: ["gpt-5.5"],
    availableModels: ["gpt-5.5", "gpt-5.6-luna"],
    planType: "plus",
    botFlagged: null,
    usage: usageParsed(),
  };
}

function usageParsed() {
  const windowMs = 2 * 60 * 1000;
  const newest = Math.floor(Date.now() / windowMs) * windowMs;
  return {
    totalRequests: 3,
    successfulRequests: 2,
    failedRequests: 1,
    windowMinutes: 2,
    windowSlots: Array.from({ length: 30 }, (_, index) => ({
      startedAtMs: newest - (29 - index) * windowMs,
      totalRequests: index >= 27 ? 1 : 0,
      successfulRequests: index === 27 || index === 29 ? 1 : 0,
      failedRequests: index === 28 ? 1 : 0,
    })),
  };
}

function accountJson(
  label: string,
  configVersion: number,
  requestsPerMinute: number | null,
  models = ["gpt-5.5"],
) {
  return {
    id: accountId,
    provider_kind: "codex",
    label,
    requests_per_minute: requestsPerMinute,
    enabled: true,
    safe_account_email: "person@example.com",
    expires_at: 1_900_000_000,
    token_version: 1,
    account_generation: 1,
    config_version: configVersion,
    selected_model_count: models.length,
    models,
    available_models: ["gpt-5.5", "gpt-5.6-luna"],
    plan_type: "plus",
    bot_flagged: null,
    usage: usageJson(),
  };
}

function usageJson() {
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

function response(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
