import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { OAuthManagement } from "./OAuthManagement";
import { clearNotifications, NotificationHost } from "@/shared/notifications";

afterEach(() => {
  clearNotifications();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("inspects, confirms, and deletes only explicitly invalid OAuth accounts", async () => {
  let deleted = false;
  const items = [
    account("invalid", "Invalid Account"),
    account("restricted", "Restricted Account"),
    account("valid", "Valid Account"),
  ];
  const fetchMock = vi.fn(
    async (input: RequestInfo | URL, init?: RequestInit) => {
      const path = String(input);
      if (path === "/api/admin/oauth/accounts") {
        return jsonResponse(configuration(deleted ? 2 : 1, deleted ? items.slice(1) : items));
      }
      if (path.endsWith("/quota") && init?.method === "GET") {
        return jsonResponse(null);
      }
      if (path.endsWith("/invalid/quota/refresh") && init?.method === "POST") {
        return errorResponse("oauth_account_authentication_failed", 502);
      }
      if (path.endsWith("/restricted/quota/refresh") && init?.method === "POST") {
        return errorResponse("oauth_account_restricted", 502);
      }
      if (path.endsWith("/valid/quota/refresh") && init?.method === "POST") {
        return jsonResponse(quota());
      }
      if (
        path === "/api/admin/oauth/accounts/invalid?expected_revision=1&expected_config_version=1" &&
        init?.method === "DELETE"
      ) {
        deleted = true;
        return jsonResponse(configuration(2, items.slice(1)));
      }
      throw new Error(`unexpected request: ${path}`);
    },
  );
  vi.stubGlobal("fetch", fetchMock);

  renderManagement();
  expect(await screen.findByText("Invalid Account")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "删除失效账号" }));

  const dialog = await screen.findByRole("alertdialog", {
    name: "删除失效账号",
  });
  expect(
    within(dialog).getByText(/已通过上游认证诊断确认 1 个 Codex 账号失效/),
  ).toBeInTheDocument();
  expect(within(dialog).getByText("目标：Invalid Account")).toBeInTheDocument();
  expect(within(dialog).getByText("另有 1 个账号无法确认，均会保留。")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "删除 Invalid Account" })).toBeDisabled();
  expect(
    fetchMock.mock.calls.filter(([, init]) => init?.method === "DELETE"),
  ).toHaveLength(0);

  fireEvent.click(within(dialog).getByRole("button", { name: "删除 1 个账号" }));
  expect(await screen.findByText("已删除 1 个无效的 Codex OAuth 账号。")).toBeInTheDocument();
  await waitFor(() => expect(screen.queryByText("Invalid Account")).not.toBeInTheDocument());
  expect(screen.getByText("Restricted Account")).toBeInTheDocument();
  expect(screen.getByText("Valid Account")).toBeInTheDocument();

  const deletePaths = fetchMock.mock.calls
    .filter(([, init]) => init?.method === "DELETE")
    .map(([input]) => String(input));
  expect(deletePaths).toEqual([
    "/api/admin/oauth/accounts/invalid?expected_revision=1&expected_config_version=1",
  ]);
});

test("keeps accounts when authentication failure is not conclusive", async () => {
  const items = [account("unverified", "Unverified Account")];
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path === "/api/admin/oauth/accounts") {
      return jsonResponse(configuration(1, items));
    }
    if (path.endsWith("/quota") && init?.method === "GET") {
      return jsonResponse(null);
    }
    if (path.endsWith("/unverified/quota/refresh") && init?.method === "POST") {
      return errorResponse("oauth_account_authentication_unverified", 502);
    }
    if (init?.method === "DELETE") {
      throw new Error("unverified account must not be deleted");
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderManagement();
  expect(await screen.findByText("Unverified Account")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "删除失效账号" }));

  expect(
    await screen.findByText(
      "未发现明确认证失效的 Codex OAuth 账号；1 个账号因其他错误无法确认，均已保留。",
    ),
  ).toBeInTheDocument();
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  expect(screen.getByText("Unverified Account")).toBeInTheDocument();
  expect(
    fetchMock.mock.calls.filter(([, init]) => init?.method === "DELETE"),
  ).toHaveLength(0);
});

function renderManagement() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: 10_000 } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/oauth"]}>
        <OAuthManagement />
        <NotificationHost />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function configuration(configRevision: number, items: unknown[]) {
  return { config_revision: configRevision, items };
}

function account(id: string, label: string) {
  return {
    id,
    provider_kind: "codex",
    label,
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
  };
}

function quota() {
  return { fetched_at: 1_900_000_000, rate_limit: null, reset_credits: null };
}

function errorResponse(code: string, status: number) {
  return new Response(
    JSON.stringify({ error: { code, message: "quota request failed" } }),
    { status, headers: { "Content-Type": "application/json" } },
  );
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function usage() {
  const intervalMs = 2 * 60 * 1_000;
  return {
    total_requests: 0,
    successful_requests: 0,
    failed_requests: 0,
    window_minutes: 2,
    window_slots: Array.from({ length: 30 }, (_, index) => ({
      started_at_ms: index * intervalMs,
      total_requests: 0,
      successful_requests: 0,
      failed_requests: 0,
    })),
  };
}
