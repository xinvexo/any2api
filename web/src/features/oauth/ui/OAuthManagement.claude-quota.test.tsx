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

test("shows every Claude usage window and refreshes the full provider set", async () => {
  let quotaReads = 0;
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const path = String(input);
      if (path === "/api/admin/oauth/accounts") {
        return jsonResponse({ config_revision: 1, items: [claudeAccount()] });
      }
      if (path === "/api/admin/oauth/accounts/claude-1/quota") {
        quotaReads += 1;
        return jsonResponse(claudeQuota());
      }
      throw new Error(`unexpected request: ${path}`);
    }),
  );
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/oauth?kind=claude"]}>
        <OAuthManagement />
        <NotificationHost />
      </MemoryRouter>
    </QueryClientProvider>,
  );

  const panel = await screen.findByRole("region", { name: "Claude 额度" });
  expect(within(panel).queryByRole("button", { name: "重置额度" })).not.toBeInTheDocument();
  expect(within(panel).queryByText("重置次数")).not.toBeInTheDocument();

  fireEvent.click(within(panel).getByRole("button", { name: "刷新额度" }));
  expect(
    await within(panel).findByRole("progressbar", { name: "5 小时限额 剩余 87.5%" }),
  ).toBeInTheDocument();
  expect(
    within(panel).getByRole("progressbar", { name: "7 天限额 剩余 66.0%" }),
  ).toBeInTheDocument();
  expect(
    within(panel).getByRole("progressbar", { name: "Sonnet 7 天限额 剩余 44.0%" }),
  ).toBeInTheDocument();
  expect(
    within(panel).getByRole("progressbar", { name: "Fable 7 天限额 剩余 22.0%" }),
  ).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "刷新全部额度" }));
  expect(await screen.findByText("已刷新全部 1 个 Claude 账号额度。")).toBeInTheDocument();
  await waitFor(() => expect(quotaReads).toBe(2));
});

function claudeAccount() {
  const windowMs = 2 * 60 * 1000;
  const newest = Math.floor(Date.now() / windowMs) * windowMs;
  return {
    id: "claude-1",
    provider_kind: "claude",
    label: "Claude One",
    requests_per_minute: null,
    enabled: true,
    safe_account_email: null,
    expires_at: null,
    token_version: 1,
    account_generation: 1,
    config_version: 1,
    selected_model_count: 1,
    models: ["claude-sonnet-4-5"],
    available_models: ["claude-sonnet-4-5"],
    plan_type: null,
    bot_flagged: null,
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

function claudeQuota() {
  return {
    fetched_at: 1_900_000_000,
    rate_limit: {
      allowed: null,
      limit_reached: null,
      windows: [
        window("five_hour", 12.5, 18_000),
        window("seven_day", 34, 604_800),
        window("seven_day_sonnet", 56, 604_800),
        window("seven_day_overage_included", 78, 604_800),
      ],
    },
    reset_credits: null,
  };
}

function window(id: string, usedPercent: number, seconds: number) {
  return {
    id,
    kind: "time",
    used_percent: usedPercent,
    limit_window_seconds: seconds,
    reset_after_seconds: null,
    reset_at: 1_900_000_300,
  };
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
