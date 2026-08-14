import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { FakeEventSource } from "@/test/fake-event-source";

import { oauthQuotaQueryOptions } from "./oauth-quota-query";
import { useOAuthQuotaChangeEvent } from "./use-oauth-quota-change-event";

afterEach(() => {
  FakeEventSource.reset();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("reloads active snapshots after reconnects and page-level change events", async () => {
  vi.stubGlobal("EventSource", FakeEventSource);
  let reads = 0;
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    expect(String(input)).toBe("/api/admin/oauth/accounts/account-1/quota");
    expect(init?.method).toBe("GET");
    reads += 1;
    return jsonResponse(quota(reads));
  });
  vi.stubGlobal("fetch", fetchMock);

  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const invalidate = vi.spyOn(client, "invalidateQueries");
  const view = render(
    <QueryClientProvider client={client}>
      <QuotaConsumer />
    </QueryClientProvider>,
  );

  expect(await screen.findByText("snapshot 1")).toBeInTheDocument();
  expect(FakeEventSource.instances).toHaveLength(1);
  expect(FakeEventSource.instances[0]?.url).toBe("/api/admin/oauth/quota-events");

  act(() => {
    FakeEventSource.instances[0]?.emit("open");
  });
  expect(await screen.findByText("snapshot 2")).toBeInTheDocument();

  act(() => {
    FakeEventSource.instances[0]?.emit("oauth_quota_changed");
  });
  expect(await screen.findByText("snapshot 3")).toBeInTheDocument();
  expect(fetchMock).toHaveBeenCalledTimes(3);
  expect(fetchMock.mock.calls.every(([, init]) => init?.method === "GET")).toBe(true);

  invalidate.mockClear();
  act(() => {
    FakeEventSource.instances[0]?.emit("oauth_refresh_diagnostic_changed");
  });
  expect(invalidate).toHaveBeenCalledWith({
    queryKey: ["oauth", "accounts"],
    refetchType: "active",
  });

  const source = FakeEventSource.instances[0];
  view.unmount();
  await waitFor(() => expect(source?.closed).toBe(true));
});

function QuotaConsumer() {
  useOAuthQuotaChangeEvent();
  const query = useQuery(oauthQuotaQueryOptions("account-1"));
  return <p>{query.data ? `snapshot ${query.data.fetchedAt}` : "no snapshot"}</p>;
}

function quota(fetchedAt: number) {
  return {
    fetched_at: fetchedAt,
    rate_limit: null,
    credits: null,
    access: null,
    reset_credits: null,
    billing: null,
    token_balance: null,
    subscription_tier: null,
    account_status: null,
    rate_card: {
      id: "openai_codex_credits_2026_08_11",
      credits_per_usd: 25,
    },
    estimates: [],
  };
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
