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

test("refreshes Codex quota and consumes one available reset credit", async () => {
  let resetCompleted = false;
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path.endsWith("/quota") && init?.method === "GET") {
      return response(quota(resetCompleted ? 0 : 1));
    }
    if (path.endsWith("/quota/reset") && init?.method === "POST") {
      expect(init.body).toBeUndefined();
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

  fireEvent.click(within(panel).getByRole("button", { name: "刷新额度" }));
  // used 37.5% → remaining 62.5% rendered as 63%
  expect(await within(panel).findByText("63%")).toBeInTheDocument();
  expect(within(panel).getByText("1")).toBeInTheDocument();
  expect(resetButton).toBeEnabled();

  fireEvent.click(resetButton);
  const dialog = await screen.findByRole("alertdialog");
  expect(dialog).toHaveTextContent("当前剩余 1 次");
  fireEvent.click(within(dialog).getByRole("button", { name: "重置额度" }));

  const notification = await screen.findByRole("status");
  expect(notification).toHaveTextContent("已重置 2 个额度窗口。");
  expect(notification.className).toContain("notification-card");
  expect(within(panel).queryByText("已重置 2 个额度窗口。")).not.toBeInTheDocument();
  await waitFor(() => expect(within(panel).getByText("0")).toBeInTheDocument());
  expect(within(panel).getByRole("button", { name: "重置额度" })).toBeDisabled();
  expect(fetchMock).toHaveBeenCalledTimes(3);
  expect(fetchMock.mock.calls.map(([path]) => String(path))).toEqual([
    "/api/admin/oauth/accounts/account-1/quota",
    "/api/admin/oauth/accounts/account-1/quota/reset",
    "/api/admin/oauth/accounts/account-1/quota",
  ]);
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
    if (path.endsWith("/quota") && init?.method === "GET") {
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
  await waitFor(() => expect(within(panel).getByText("0")).toBeInTheDocument());
  expect(fetchMock).toHaveBeenCalledTimes(2);
});

test("keeps a command refresh alive when the virtualized panel unmounts", async () => {
  const client = createClient();
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

test("clears stale quota after reset refresh failure and recovers on refresh", async () => {
  const client = createClient();
  let quotaReads = 0;
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path.endsWith("/quota") && init?.method === "GET") {
      quotaReads += 1;
      return quotaReads === 2
        ? errorResponse("oauth_quota_upstream_failed", 502)
        : response(quota(quotaReads === 1 ? 1 : 0));
    }
    if (path.endsWith("/quota/reset") && init?.method === "POST") {
      return response({ windows_reset: 1 });
    }
    throw new Error(`unexpected request: ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderPanel(client);
  let panel = screen.getByRole("region", { name: "Codex 额度" });
  fireEvent.click(within(panel).getByRole("button", { name: "刷新额度" }));
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
  expect(within(panel).getByText("额度尚未刷新")).toBeInTheDocument();
  expect(within(panel).queryByText("63%")).not.toBeInTheDocument();
  expect(client.getQueryData(oauthQueryKeys.quota("account-1"))).toBeUndefined();

  fireEvent.click(within(panel).getByRole("button", { name: "刷新额度" }));
  await waitFor(() => expect(within(panel).getByText("0")).toBeInTheDocument());
  panel = screen.getByRole("region", { name: "Codex 额度" });
  expect(
    within(panel).queryByText("额度已重置，但最新额度读取失败。"),
  ).not.toBeInTheDocument();
});

test("shows only a real Grok exhaustion observation with its actual limit", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => response({
      fetched_at: 1_900_000_000,
      rate_limit: null,
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
    })),
  );

  renderGrokPanel();
  const panel = screen.getByRole("region", { name: "Grok 额度" });
  fireEvent.click(within(panel).getByRole("button", { name: "刷新额度" }));

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
    vi.fn(async () => errorResponse("oauth_account_authentication_failed", 502)),
  );

  renderGrokPanel();
  const panel = screen.getByRole("region", { name: "Grok 额度" });
  fireEvent.click(within(panel).getByRole("button", { name: "刷新额度" }));

  expect(
    await within(panel).findByText("账号认证已失效：刷新 Token 后仍被上游拒绝。"),
  ).toBeInTheDocument();
  expect(within(panel).getByText("额度尚未刷新")).toBeInTheDocument();
});

function createClient() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
}

function renderPanel(client = createClient()) {
  return render(
    <QueryClientProvider client={client}>
      <OAuthQuotaPanel
        accountId="account-1"
        accountLabel="Primary Codex"
        provider="codex"
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
    reset_credits: {
      available_count: availableCount,
      expires_at: availableCount > 0 ? ["2026-07-30T00:00:00Z"] : [],
    },
  };
}

function response(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function errorResponse(code: string, status: number) {
  return new Response(
    JSON.stringify({ error: { code, message: "quota request failed" } }),
    { status, headers: { "Content-Type": "application/json" } },
  );
}
