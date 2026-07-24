import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import type { OAuthAccount } from "../api/oauth-contracts";
import { OAuthAccounts } from "./OAuthAccounts";

afterEach(() => vi.restoreAllMocks());

test("lists and edits OAuth accounts without receiving token material", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path.endsWith(`/api/admin/oauth/accounts/${accountId}`) && init?.method === "PATCH") {
      expect(JSON.parse(String(init.body))).toEqual({
        expected_revision: 2,
        expected_config_version: 1,
        label: "Renamed Codex",
        max_concurrency: 3,
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
  fireEvent.change(screen.getByLabelText("最大并发"), { target: { value: "3" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
});

test("shows kind-scoped empty state without a session panel", () => {
  renderAccounts([]);
  expect(screen.getByText("还没有 Codex OAuth 账号")).toBeInTheDocument();
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
    maxConcurrency: 1,
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
  };
}

function accountJson(label: string, configVersion: number, maxConcurrency: number) {
  return {
    id: accountId,
    provider_kind: "codex",
    label,
    max_concurrency: maxConcurrency,
    enabled: true,
    safe_account_email: "person@example.com",
    expires_at: 1_900_000_000,
    token_version: 1,
    account_generation: 1,
    config_version: configVersion,
    selected_model_count: 1,
    models: ["gpt-5.5"],
    available_models: ["gpt-5.5", "gpt-5.6-luna"],
    plan_type: "plus",
  };
}

function response(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
