import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { GlobalProxySettings } from "./GlobalProxySettings";

const direct = {
  id: "00000000-0000-0000-0000-000000000000",
  name: "DIRECT",
  kind: "direct",
  host: null,
  port: null,
  username: null,
  password_configured: false,
  authentication_version: 0,
  enabled: true,
  built_in: true,
  config_version: 1,
};

const custom = {
  id: "a81bf8f8-8fb4-45f0-926d-1cfda84884f5",
  name: "Hong Kong",
  kind: "http",
  host: "proxy.example.com",
  port: 8080,
  username: null,
  password_configured: false,
  authentication_version: 0,
  enabled: true,
  built_in: false,
  config_version: 1,
};

afterEach(() => vi.restoreAllMocks());

test("shows global proxy inheritance copy and applies a new global exit", async () => {
  let current = {
    config_revision: 1,
    global_proxy_id: direct.id,
    items: [direct, custom],
  };
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PUT" || init?.method === "POST" || init?.method === "PATCH") {
      current = {
        config_revision: 2,
        global_proxy_id: custom.id,
        items: [direct, custom],
      };
      return jsonResponse(current);
    }
    return jsonResponse(current);
  });

  renderSettings();

  expect(await screen.findByText("全局出口代理")).toBeInTheDocument();
  expect(screen.getByText(/Credential 绑定 DIRECT 时会继承此出口/)).toBeInTheDocument();

  fireEvent.change(screen.getByLabelText(/当前出口/), { target: { value: custom.id } });
  fireEvent.click(screen.getByRole("button", { name: "应用" }));

  await waitFor(() => {
    expect(screen.getByLabelText(/当前出口 · Hong Kong/)).toBeInTheDocument();
  });
  expect(fetchMock.mock.calls.some(([, init]) => init?.method)).toBe(true);
});

function renderSettings() {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return render(
    <QueryClientProvider client={client}>
      <GlobalProxySettings />
    </QueryClientProvider>,
  );
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
