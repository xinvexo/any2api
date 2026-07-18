import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { GatewayApiKeyManagement } from "./GatewayApiKeyManagement";

afterEach(() => vi.restoreAllMocks());

test("creates a gateway key without retaining its token in caches or URL", async () => {
  const token = `a2k_v1_${"b".repeat(43)}`;
  let configuration: Record<string, unknown> = { config_revision: 1, items: [] };
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "POST") {
      configuration = {
        config_revision: 2,
        items: [
          {
            id: "key-1",
            name: "Desktop",
            token_prefix: token.slice(0, 16),
            token_version: 1,
            config_version: 1,
            enabled: true,
            revoked_at: null,
            created_at: "2026-07-19 10:00:00",
            last_used_at: null,
          },
        ],
        token,
      };
      return jsonResponse(configuration);
    }
    return jsonResponse(configuration);
  });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/keys?editor=new"]}>
        <GatewayApiKeyManagement />
        <LocationProbe />
      </MemoryRouter>
    </QueryClientProvider>,
  );

  fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "Desktop" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(await screen.findByLabelText("本次生成的网关密钥")).toHaveValue(token);
  expect(fetchMock.mock.calls.some(([, init]) => init?.method === "POST")).toBe(true);
  expect(JSON.stringify(client.getQueryCache().getAll().map((query) => query.state.data))).not.toContain(token);
  expect(JSON.stringify(client.getMutationCache().getAll())).not.toContain(token);
  expect(screen.getByTestId("location")).not.toHaveTextContent(token);

  fireEvent.click(screen.getByRole("button", { name: "关闭密钥回执" }));
  await waitFor(() => expect(screen.queryByLabelText("本次生成的网关密钥")).not.toBeInTheDocument());
  expect(document.body.innerHTML).not.toContain(token);
});

function LocationProbe() {
  const location = useLocation();
  return <span data-testid="location" hidden>{`${location.pathname}${location.search}`}</span>;
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
