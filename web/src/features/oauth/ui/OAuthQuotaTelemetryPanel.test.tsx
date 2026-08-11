import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { OAuthQuotaPanel } from "./OAuthQuotaPanel";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("shows real Credits as dollars and inline epoch capacity", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => response(quotaWithCreditsAndEstimate())),
  );

  render(
    <QueryClientProvider client={createClient()}>
      <OAuthQuotaPanel
        accountId="account-1"
        accountLabel="Primary Codex"
        provider="codex"
      />
    </QueryClientProvider>,
  );
  const panel = screen.getByRole("region", { name: "Codex 额度" });
  const credits = await within(panel).findByText("$9.9371");
  expect(within(panel).getByText("Credits")).toBeInTheDocument();
  expect(within(panel).queryByText("248.4272780000 Credits")).not.toBeInTheDocument();
  expect(credits).toHaveAttribute(
    "title",
    "248.4272780000 Credits · 25 Credits = $1",
  );
  const estimate = within(panel).getByText("$0.38/$1.00");
  expect(estimate.parentElement).toContainElement(within(panel).getByText("63%"));
  expect(estimate.getAttribute("title")).toContain("剩余 $0.63");
  expect(estimate.getAttribute("title")).toContain(
    "区间本地消费 0.25 Credits · 官方使用率变化 1%",
  );
  expect(estimate.getAttribute("title")).toContain("置信度 稳定 · 3 个样本 · Epoch 7");
  expect(estimate.getAttribute("title")).toContain("费率卡 openai_codex_credits_2026_08_11");
  expect(within(panel).queryByText(/区间本地消费/)).not.toBeInTheDocument();
});

function createClient() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
}

function quotaWithCreditsAndEstimate() {
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
    credits: {
      has_credits: true,
      unlimited: false,
      balance: "248.4272780000",
    },
    access: {
      spend_control_reached: false,
      reached_type: "rate_limit_reached",
    },
    reset_credits: {
      available_count: 1,
      expires_at: ["2026-07-30T00:00:00Z"],
    },
    estimates: [{
      window_id: "primary",
      window_kind: "time",
      limit_window_seconds: 18_000,
      window_reset_at: 1_900_000_300,
      epoch: 7,
      epoch_started_at: 1_899_999_000,
      confidence: "stable",
      estimated_capacity_credits: 25,
      estimated_used_credits: 9.375,
      estimated_remaining_credits: 15.625,
      sample_count: 3,
      relative_mad: 0.01,
      latest_interval: {
        status: "valid_sample",
        started_at: 1_899_999_700,
        ended_at: 1_900_000_000,
        delta_used_percent: 1,
        local_cost_credits: 0.25,
        unpriced_request_count: 0,
        queue_dropped_request_logs: 0,
        storage_failed_request_logs: 0,
        pruned_request_logs: 0,
      },
      rate_cards: ["openai_codex_credits_2026_08_11"],
    }],
  };
}

function response(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
