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
      binding_count: 16,
      creating_count: 1,
      credential_counts: [],
      bindings: [],
    }),
  );

  const rendered = renderOverview();

  expect(await screen.findByRole("heading", { name: "会话绑定" })).toBeInTheDocument();
  expect(screen.getByText("当前绑定")).toBeInTheDocument();
  expect(screen.getByText("16")).toBeInTheDocument();
  expect(screen.getByText("正在创建")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "调整策略" })).toHaveAttribute(
    "href",
    "/settings/routing",
  );
  expect(String(fetchMock.mock.calls[0]?.[0])).toContain("/api/admin/affinity?limit=0");
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
