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
  expect(within(panel).queryByRole("button", { name: "重置额度" })).not.toBeInTheDocument();
  expect(within(panel).queryByText("重置次数")).not.toBeInTheDocument();

  fireEvent.click(within(panel).getByRole("button", { name: "刷新额度" }));
  expect(await within(panel).findByText("75%")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "刷新全部额度" }));
  expect(await screen.findByRole("status")).toHaveTextContent(
    "已刷新全部 1 个 Grok 账号额度。",
  );
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
    enabled: true,
    safe_account_email: null,
    expires_at: null,
    token_version: 1,
    account_generation: 1,
    config_version: 1,
    selected_model_count: 1,
    models: ["grok-4.5"],
    available_models: ["grok-4.5"],
    plan_type: null,
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
    rate_limit: {
      allowed: true,
      limit_reached: false,
      primary_window: {
        used_percent: 25,
        limit_window_seconds: 604_800,
        reset_after_seconds: 300,
        reset_at: 1_900_000_300,
      },
      secondary_window: null,
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
