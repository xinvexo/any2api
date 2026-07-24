import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { OAuthAccounts } from "./OAuthAccounts";

afterEach(() => vi.restoreAllMocks());

test("lists and edits OAuth accounts without receiving token material", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path === "/api/admin/oauth/accounts" && init?.method === "GET") {
      return response(configuration(2, "Primary Codex", 1));
    }
    if (path.endsWith(`/api/admin/oauth/accounts/${accountId}`) && init?.method === "PATCH") {
      expect(JSON.parse(String(init.body))).toEqual({
        expected_revision: 2,
        expected_config_version: 1,
        label: "Renamed Codex",
        max_concurrency: 3,
        enabled: true,
      });
      expect(String(init.body)).not.toContain("token");
      return response(configuration(3, "Renamed Codex", 2));
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderAccounts();
  expect(await screen.findByText("Primary Codex")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "编辑" }));
  fireEvent.change(await screen.findByLabelText("账号名称"), {
    target: { value: "Renamed Codex" },
  });
  fireEvent.change(screen.getByLabelText("最大并发"), { target: { value: "3" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("Renamed Codex")).toBeInTheDocument();
  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));
});

function renderAccounts() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/oauth"]}>
        <OAuthAccounts />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const accountId = "fdcb6e74-820f-4d84-9df6-38af2b031feb";

function configuration(configRevision: number, label: string, configVersion: number) {
  return {
    config_revision: configRevision,
    items: [
      {
        id: accountId,
        provider_kind: "codex",
        label,
        max_concurrency: configRevision === 2 ? 1 : 3,
        enabled: true,
        safe_account_email: "person@example.com",
        expires_at: 1_900_000_000,
        token_version: 1,
        account_generation: 1,
        config_version: configVersion,
        selected_model_count: 1,
        models: ["gpt-5.5"],
      },
    ],
  };
}

function response(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
