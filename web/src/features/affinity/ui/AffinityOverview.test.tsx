import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { AffinityOverview } from "./AffinityOverview";

afterEach(() => vi.restoreAllMocks());

test("renders only aggregate affinity counts and links to routing settings", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
    jsonResponse({
      config_revision: 7,
      affinity_enabled: false,
      active_session_count: 0,
      creating_session_count: 0,
    }),
  );

  const rendered = renderOverview();

  expect(await screen.findByRole("heading", { name: "活动会话" })).toBeInTheDocument();
  expect(
    screen.getByText("显式会话粘性已关闭；Response ID 续接仍按原目标处理，但不计入会话数。"),
  ).toBeInTheDocument();
  expect(screen.getByText("活动显式会话")).toBeInTheDocument();
  expect(screen.getByText("已关闭")).toBeInTheDocument();
  expect(screen.getByText("建立中显式会话")).toBeInTheDocument();
  expect(screen.getByText("显式会话粘性未启用")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "调整策略" })).toHaveAttribute(
    "href",
    "/settings/routing",
  );
  expect(String(fetchMock.mock.calls[0]?.[0])).toContain("/api/admin/affinity");
  expect(screen.queryByText("Credential 绑定分布")).not.toBeInTheDocument();
  expect(rendered.container.querySelector(".rounded-\\[14px\\]")).toBeNull();
});

function renderOverview() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <MemoryRouter>
      <QueryClientProvider client={client}>
        <AffinityOverview />
      </QueryClientProvider>
    </MemoryRouter>,
  );
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
