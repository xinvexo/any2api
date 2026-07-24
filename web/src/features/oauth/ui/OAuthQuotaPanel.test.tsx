import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { OAuthQuotaPanel } from "./OAuthQuotaPanel";

afterEach(() => vi.restoreAllMocks());

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

  render(<OAuthQuotaPanel accountId="account-1" accountLabel="Primary Codex" />);
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

  expect(await within(panel).findByText("已重置 2 个额度窗口。")).toBeInTheDocument();
  await waitFor(() => expect(within(panel).getByText("0")).toBeInTheDocument());
  expect(within(panel).getByRole("button", { name: "重置额度" })).toBeDisabled();
  expect(fetchMock).toHaveBeenCalledTimes(3);
  expect(fetchMock.mock.calls.map(([path]) => String(path))).toEqual([
    "/api/admin/oauth/accounts/account-1/quota",
    "/api/admin/oauth/accounts/account-1/quota/reset",
    "/api/admin/oauth/accounts/account-1/quota",
  ]);
});

function quota(availableCount: number) {
  return {
    fetched_at: 1_900_000_000,
    rate_limit: {
      allowed: true,
      limit_reached: false,
      primary_window: {
        used_percent: 37.5,
        limit_window_seconds: 18_000,
        reset_after_seconds: 300,
        reset_at: 1_900_000_300,
      },
      secondary_window: null,
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
