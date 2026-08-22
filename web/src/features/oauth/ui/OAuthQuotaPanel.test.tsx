import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { parseOAuthQuotaSnapshot } from "../api/oauth-quota-contracts";
import { oauthQueryKeys } from "../model/oauth-query-keys";
import { refreshOAuthAccountQuota } from "../model/oauth-quota-query";
import { OAuthQuotaPanel } from "./OAuthQuotaPanel";
import { clearNotifications, NotificationHost } from "@/shared/notifications";

afterEach(() => {
  clearNotifications();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("restores a persisted quota snapshot without an upstream refresh", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    expect(String(input)).toBe("/api/admin/oauth/accounts/account-1/quota");
    expect(init?.method).toBe("GET");
    return response(quota(1));
  });
  vi.stubGlobal("fetch", fetchMock);

  renderPanel();
  const panel = screen.getByRole("region", { name: "Codex 额度" });
  expect(await within(panel).findByText("63%")).toBeInTheDocument();
  expect(within(panel).queryByText(/最后更新/)).not.toBeInTheDocument();
  expect(fetchMock).toHaveBeenCalledTimes(1);
});

test("refreshes Codex quota and consumes one available reset credit", async () => {
  let resetCompleted = false;
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path.endsWith("/quota") && init?.method === "GET") {
      return response(null);
    }
    if (path.endsWith("/quota/refresh") && init?.method === "POST") {
      expect(init.body).toBeUndefined();
      return response(quota(resetCompleted ? 0 : 1));
    }
    if (path.endsWith("/quota/reset") && init?.method === "POST") {
      expect(
        JSON.parse(String(init.body)).redeem_request_id,
      ).toMatch(/^[0-9a-f-]{36}$/);
      resetCompleted = true;
      return response({ windows_reset: 2 });
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderPanel();
  const panel = screen.getByRole("region", { name: "Codex 额度" });
  const resetButton = within(panel).getByRole("button", { name: "重置额度" });
  expect(resetButton).toBeDisabled();
  expect(resetButton).toHaveClass("bg-transparent", "text-danger");

  const refreshButton = within(panel).getByRole("button", { name: "刷新额度" });
  expect(refreshButton).toHaveClass("bg-transparent", "text-secondary");
  await waitFor(() => expect(refreshButton).toBeEnabled());
  fireEvent.click(refreshButton);
  // used 37.5% → remaining 62.5% rendered as 63%
  expect(await within(panel).findByText("63%")).toBeInTheDocument();
  expect(await screen.findByText("已刷新「Primary Codex」的额度")).toBeInTheDocument();
  const resetCount = within(panel).getByText("可重置");
  expect(resetCount).toHaveTextContent("可重置 1");
  expect(resetCount).not.toHaveAttribute("title");
  fireEvent.mouseEnter(resetCount);
  const resetExpiryTooltip = screen.getByRole("tooltip");
  expect(resetExpiryTooltip).toHaveTextContent(
    `最早到期：${new Date("2026-07-30T00:00:00Z").toLocaleString()}`,
  );
  expect(resetCount).toHaveAttribute("aria-describedby", resetExpiryTooltip.id);
  expect(within(panel).queryByText("重置次数")).not.toBeInTheDocument();
  expect(within(panel).getByRole("button", { name: "刷新额度" })).toHaveAttribute(
    "title",
    "刷新额度",
  );
  expect(resetButton).toHaveClass("text-danger");
  expect(resetButton).toBeEnabled();

  fireEvent.click(resetButton);
  const dialog = await screen.findByRole("alertdialog");
  expect(dialog).toHaveTextContent("当前剩余 1 次");
  fireEvent.click(within(dialog).getByRole("button", { name: "重置额度" }));

  expect(await screen.findByText("已重置 2 个额度窗口。")).toBeInTheDocument();
  expect(within(panel).queryByText("已重置 2 个额度窗口。")).not.toBeInTheDocument();
  await waitFor(() =>
    expect(within(panel).getByText("可重置")).toHaveTextContent("可重置 0"),
  );
  expect(within(panel).getByRole("button", { name: "重置额度" })).toBeDisabled();
  expect(fetchMock).toHaveBeenCalledTimes(4);
  expect(fetchMock.mock.calls.map(([path, init]) => [String(path), init?.method])).toEqual([
    ["/api/admin/oauth/accounts/account-1/quota", "GET"],
    ["/api/admin/oauth/accounts/account-1/quota/refresh", "POST"],
    ["/api/admin/oauth/accounts/account-1/quota/reset", "POST"],
    ["/api/admin/oauth/accounts/account-1/quota/refresh", "POST"],
  ]);
});

test("warns when quota refresh cannot synchronize the model catalog", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path.endsWith("/quota") && init?.method === "GET") {
      return response(null);
    }
    if (path.endsWith("/quota/refresh") && init?.method === "POST") {
      return response({ ...quota(1), model_catalog_refreshed: false });
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderPanel();
  const panel = screen.getByRole("region", { name: "Codex 额度" });
  const refreshButton = within(panel).getByRole("button", { name: "刷新额度" });
  await waitFor(() => expect(refreshButton).toBeEnabled());
  fireEvent.click(refreshButton);

  expect(
    await screen.findByText("已刷新「Primary Codex」的额度，但模型目录同步失败。"),
  ).toBeInTheDocument();
  expect(
    await within(panel).findByText("额度已刷新，但模型目录同步失败。"),
  ).toBeInTheDocument();
});

test("keeps reset pending when a virtualized account panel remounts", async () => {
  const client = createClient();
  client.setQueryData(
    oauthQueryKeys.quota("account-1"),
    parseOAuthQuotaSnapshot(quota(1)),
  );
  const resetResponse = deferred<Response>();
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path.endsWith("/quota/reset") && init?.method === "POST") {
      return resetResponse.promise;
    }
    if (path.endsWith("/quota/refresh") && init?.method === "POST") {
      return response(quota(0));
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  const first = renderPanel(client);
  let panel = screen.getByRole("region", { name: "Codex 额度" });
  fireEvent.click(within(panel).getByRole("button", { name: "重置额度" }));
  fireEvent.click(
    within(await screen.findByRole("alertdialog")).getByRole("button", {
      name: "重置额度",
    }),
  );
  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
  first.unmount();

  renderPanel(client);
  panel = screen.getByRole("region", { name: "Codex 额度" });
  expect(within(panel).getByRole("button", { name: "重置额度" })).toBeDisabled();

  resetResponse.resolve(response({ windows_reset: 1 }));
  await waitFor(() =>
    expect(within(panel).getByText("可重置")).toHaveTextContent("可重置 0"),
  );
  expect(fetchMock).toHaveBeenCalledTimes(2);
});

test("keeps a command refresh alive when the virtualized panel unmounts", async () => {
  const client = createClient();
  client.setQueryData(oauthQueryKeys.quota("account-1"), null);
  const quotaResponse = deferred<Response>();
  let aborted = false;
  const fetchMock = vi.fn(
    async (_input: RequestInfo | URL, init?: RequestInit) => {
      init?.signal?.addEventListener("abort", () => {
        aborted = true;
      });
      return quotaResponse.promise;
    },
  );
  vi.stubGlobal("fetch", fetchMock);

  const panel = renderPanel(client);
  const refresh = refreshOAuthAccountQuota(client, "account-1");
  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
  panel.unmount();

  expect(aborted).toBe(false);
  quotaResponse.resolve(response(quota(1)));
  await expect(refresh).resolves.toBeDefined();
  expect(client.getQueryData(oauthQueryKeys.quota("account-1"))).toBeDefined();
});

test("keeps the last quota after reset refresh failure and recovers on refresh", async () => {
  const client = createClient();
  let quotaRefreshes = 0;
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path.endsWith("/quota") && init?.method === "GET") {
      return response(null);
    }
    if (path.endsWith("/quota/refresh") && init?.method === "POST") {
      quotaRefreshes += 1;
      return quotaRefreshes === 2
        ? errorResponse("oauth_quota_upstream_failed", 502)
        : response(quota(quotaRefreshes === 1 ? 1 : 0));
    }
    if (path.endsWith("/quota/reset") && init?.method === "POST") {
      return response({ windows_reset: 1 });
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderPanel(client);
  let panel = screen.getByRole("region", { name: "Codex 额度" });
  const refreshButton = within(panel).getByRole("button", { name: "刷新额度" });
  await waitFor(() => expect(refreshButton).toBeEnabled());
  fireEvent.click(refreshButton);
  expect(await within(panel).findByText("63%")).toBeInTheDocument();

  fireEvent.click(within(panel).getByRole("button", { name: "重置额度" }));
  fireEvent.click(
    within(await screen.findByRole("alertdialog")).getByRole("button", {
      name: "重置额度",
    }),
  );

  expect(
    await within(panel).findByText("额度已重置，但最新额度读取失败。"),
  ).toBeInTheDocument();
  expect(within(panel).getByText("63%")).toBeInTheDocument();
  expect(client.getQueryData(oauthQueryKeys.quota("account-1"))).toBeDefined();

  fireEvent.click(within(panel).getByRole("button", { name: "刷新额度" }));
  await waitFor(() =>
    expect(within(panel).getByText("可重置")).toHaveTextContent("可重置 0"),
  );
  panel = screen.getByRole("region", { name: "Codex 额度" });
  expect(
    within(panel).queryByText("额度已重置，但最新额度读取失败。"),
  ).not.toBeInTheDocument();
});

test("shows only a real Grok exhaustion observation with its actual limit", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => response(
      init?.method === "GET" ? null : {
      fetched_at: 1_900_000_000,
      rate_limit: null,
      credits: null,
      access: null,
      reset_credits: null,
      billing: null,
      token_balance: {
        source: "upstream",
        used: 1_065_387,
        limit: 1_000_000,
        remaining: 0,
        window_seconds: null,
      },
      subscription_tier: "Free",
      account_status: {
        authentication: "valid",
        user_blocked_reason: null,
        team_blocked_reasons: [],
        quota_exhaustion: {
          observed_at: 1_900_000_000,
          used: 1_065_387,
          limit: 1_000_000,
        },
      },
      rate_card: null,
      estimates: [],
      model_catalog_refreshed: true,
    })),
  );

  renderGrokPanel();
  const panel = screen.getByRole("region", { name: "Grok 额度" });
  const refreshButton = within(panel).getByRole("button", { name: "刷新额度" });
  await waitFor(() => expect(refreshButton).toBeEnabled());
  fireEvent.click(refreshButton);

  expect(await within(panel).findByText("0 / 1,000,000")).toBeInTheDocument();
  expect(within(panel).getByText("Token 余额 · 上游真实观测")).toBeInTheDocument();
  expect(within(panel).queryByText(/已用 1,065,387/)).not.toBeInTheDocument();
  expect(within(panel).queryByText("xAI 用户限制")).not.toBeInTheDocument();
  expect(within(panel).queryByText("未报告")).not.toBeInTheDocument();
  expect(within(panel).queryByText("认证状态")).not.toBeInTheDocument();
  expect(within(panel).queryByText("100%")).not.toBeInTheDocument();
});

test("reports a Grok OAuth token rejected after refresh as invalid", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) =>
      init?.method === "GET"
        ? response(null)
        : errorResponse(
            "oauth_refreshed_access_token_rejected",
            502,
            refreshDiagnostic(),
          )),
  );

  renderGrokPanel();
  const panel = screen.getByRole("region", { name: "Grok 额度" });
  const refreshButton = within(panel).getByRole("button", { name: "刷新额度" });
  await waitFor(() => expect(refreshButton).toBeEnabled());
  fireEvent.click(refreshButton);

  expect(
    await within(panel).findByText(
      /Token 已成功刷新.*阶段：刷新后认证复核.*错误：新 Access Token 仍被上游 401 拒绝/,
    ),
  ).toBeInTheDocument();
  expect(within(panel).getByText("额度尚未刷新")).toBeInTheDocument();
});

test("suppresses an error already rendered by the account diagnostic", async () => {
  const fetchMock = vi.fn(async () =>
    errorResponse(
      "oauth_refresh_permanently_rejected",
      502,
      refreshDiagnostic(),
    ));
  vi.stubGlobal("fetch", fetchMock);

  renderPanel(createClient(), false);
  const panel = screen.getByRole("region", { name: "Codex 额度" });
  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
  await waitFor(() =>
    expect(within(panel).getByRole("button", { name: "刷新额度" })).toBeEnabled(),
  );

  expect(within(panel).queryByRole("alert")).not.toBeInTheDocument();
  expect(within(panel).queryByText(/Refresh Endpoint/)).not.toBeInTheDocument();
});

function createClient() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
}

function renderPanel(client = createClient(), showError = true) {
  return render(
    <QueryClientProvider client={client}>
      <OAuthQuotaPanel
        accountId="account-1"
        accountLabel="Primary Codex"
        provider="codex"
        showError={showError}
      />
      <NotificationHost />
    </QueryClientProvider>,
  );
}

function renderGrokPanel() {
  return render(
    <QueryClientProvider client={createClient()}>
      <OAuthQuotaPanel
        accountId="grok-1"
        accountLabel="Grok Free"
        provider="grok"
      />
      <NotificationHost />
    </QueryClientProvider>,
  );
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

function quota(availableCount: number) {
  return {
    fetched_at: 1_900_000_000,
    rate_limit: {
      allowed: true,
      limit_reached: false,
      windows: [{
        id: "primary",
        kind: "time",
        used_percent: 37.5,
        limit_window_seconds: 18_000,
        reset_after_seconds: 300,
        reset_at: 1_900_000_300,
      }],
    },
    credits: null,
    access: null,
    reset_credits: {
      available_count: availableCount,
      expires_at: availableCount > 0 ? ["2026-07-30T00:00:00Z"] : [],
    },
    billing: null,
    token_balance: null,
    subscription_tier: null,
    account_status: null,
    rate_card: {
      id: "openai_codex_credits_2026_08_11",
      credits_per_usd: 25,
    },
    estimates: [],
    model_catalog_refreshed: true,
  };
}

function response(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function errorResponse(code: string, status: number, diagnostic?: unknown) {
  return new Response(
    JSON.stringify({ error: { code, message: "quota request failed", diagnostic } }),
    { status, headers: { "Content-Type": "application/json" } },
  );
}

function refreshDiagnostic() {
  return {
    token_version: 2,
    trigger: "authentication_failure",
    stage: "verify_authentication",
    reason: "refreshed_access_token_rejected",
    upstream_status: 401,
    failure_scope: null,
    occurred_at: 1_900_000_000,
    reauthorization_required: true,
  };
}
