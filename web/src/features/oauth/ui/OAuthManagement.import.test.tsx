import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { OAuthManagement } from "./OAuthManagement";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("opens OAuth JSON import in a right drawer", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      if (String(input) === "/api/admin/oauth/accounts") {
        return new Response(JSON.stringify({ config_revision: 1, items: [] }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
      throw new Error(`unexpected request: ${String(input)}`);
    }),
  );
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/oauth"]}>
        <OAuthManagement />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  await screen.findByText("还没有 Codex OAuth 账号");

  fireEvent.click(screen.getByRole("button", { name: "导入 JSON" }));

  expect(await screen.findByRole("dialog", { name: "导入 OAuth JSON" })).toBeInTheDocument();
  expect(screen.getByLabelText("OAuth JSON 文件")).toHaveAttribute("multiple");
  expect(screen.getByText(/兼容 CLIProxyAPI 与 Sub2API/)).toBeInTheDocument();
});
