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

test("shows and refreshes Grok quota without a reset action", async () => {
  let quotaReads = 0;
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const path = String(input);
      if (path === "/api/admin/oauth/accounts") {
        return jsonResponse({ config_revision: 1, items: [grokAccount()] });
      }
      if (path === "/api/admin/oauth/accounts/grok-1/quota") {
        quotaReads += 1;
        return jsonResponse(grokQuota());
      }
      throw new Error(`unexpected request: ${path}`);
    }),
  );
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/oauth?kind=grok"]}>
        <OAuthManagement />
        <NotificationHost />
      </MemoryRouter>
    </QueryClientProvider>,
  );

  const panel = await screen.findByRole("region", { name: "Grok 额度" });
  const disabledBadge = screen.getByText("已停用");
  const botIcon = screen.getByRole("img", { name: "机器人账号" });
  expect(disabledBadge.nextElementSibling).toBe(botIcon);
  expect(screen.queryByText("Build 已标记")).not.toBeInTheDocument();
  expect(within(panel).queryByRole("button", { name: "重置额度" })).not.toBeInTheDocument();
  expect(within(panel).queryByText("重置次数")).not.toBeInTheDocument();
  expect(within(panel).getByRole("button", { name: "刷新额度" })).toHaveAttribute(
    "title",
    "刷新额度",
  );
  expect(screen.getByRole("button", { name: "刷新全部额度" })).not.toHaveAttribute("title");

  fireEvent.click(within(panel).getByRole("button", { name: "刷新额度" }));
  expect(await screen.findByText("Free")).toBeInTheDocument();
  expect(within(panel).queryByText("Free")).not.toBeInTheDocument();
  expect(within(panel).queryByText("当前套餐")).not.toBeInTheDocument();
  expect(within(panel).queryByText("认证状态")).not.toBeInTheDocument();
  expect(within(panel).queryByText("有效（本次刷新）")).not.toBeInTheDocument();
  expect(within(panel).queryByText("Build 机器人标记")).not.toBeInTheDocument();
  expect(within(panel).getByText("BLOCKED_REASON_BILLING")).toBeInTheDocument();
  expect(within(panel).getByText("BLOCKED_REASON_NO_LOGS")).toBeInTheDocument();
  expect(screen.getByText("xAI 受限")).toBeInTheDocument();
  expect(within(panel).getByText("Token 余额 · 上游真实观测")).toBeInTheDocument();
  expect(within(panel).getByText("1,750,000 / 2,000,000")).toBeInTheDocument();
  expect(within(panel).queryByText(/已用 250,000/)).not.toBeInTheDocument();
  expect(within(panel).queryByText(/滚动 1 天/)).not.toBeInTheDocument();
  expect(within(panel).queryByText("100%")).not.toBeInTheDocument();
  expect(within(panel).queryByText("周限额")).not.toBeInTheDocument();
  expect(within(panel).getByText("$0.00")).toBeInTheDocument();
  expect(within(panel).queryByText("按量使用")).not.toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "刷新全部额度" }));
  expect(await screen.findByText("已刷新全部 1 个 Grok 账号额度。")).toBeInTheDocument();
  await waitFor(() => expect(quotaReads).toBe(2));
});

function grokAccount() {
  const windowMs = 2 * 60 * 1000;
  const newest = Math.floor(Date.now() / windowMs) * windowMs;
  return {
    id: "grok-1",
    provider_kind: "grok",
    label: "Grok One",
    requests_per_minute: null,
    enabled: false,
    safe_account_email: null,
    expires_at: null,
    token_version: 1,
    account_generation: 1,
    config_version: 1,
    selected_model_count: 1,
    models: ["grok-4.5"],
    available_models: ["grok-4.5"],
    plan_type: null,
    bot_flagged: true,
    usage: {
      total_requests: 0,
      successful_requests: 0,
      failed_requests: 0,
      window_minutes: 2,
      window_slots: Array.from({ length: 30 }, (_, index) => ({
        started_at_ms: newest - (29 - index) * windowMs,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
      })),
    },
  };
}

function grokQuota() {
  return {
    fetched_at: 1_900_000_000,
    rate_limit: null,
    billing: {
      currency: "USD",
      prepaid_balance_minor: 0,
      on_demand_used_minor: 0,
      on_demand_cap_minor: 0,
      is_unified_billing_user: true,
    },
    token_balance: {
      source: "upstream",
      used: 250_000,
      limit: 2_000_000,
      remaining: 1_750_000,
      window_seconds: null,
    },
    subscription_tier: "Free",
    account_status: {
      authentication: "valid",
      user_blocked_reason: "BLOCKED_REASON_BILLING",
      team_blocked_reasons: ["BLOCKED_REASON_NO_LOGS"],
      quota_exhaustion: null,
    },
    reset_credits: null,
  };
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
