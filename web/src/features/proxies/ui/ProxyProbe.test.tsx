import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { ProxyManagement } from "./ProxyManagement";

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

afterEach(() => vi.restoreAllMocks());

test("tests generic connectivity without loading or sending a Provider Endpoint", async () => {
  const proxy = customProxy();
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = requestPath(input);
    if (path.endsWith(`/proxies/${proxy.id}/test`) && init?.method === "POST") {
      return jsonResponse(proxyTestResult(proxy.id));
    }
    return jsonResponse(configuration([direct, proxy]));
  });

  renderManagement();

  expect(await screen.findByText(proxy.name)).toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "代理测试目标" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: `测试 ${proxy.name}` }));

  const status = await screen.findByRole("status", { name: /可达 · HTTP 204 · 18 ms/ });
  expect(within(status).getByText("成功")).toBeInTheDocument();
  expect(within(status).getByText("18 ms")).toBeInTheDocument();
  expect(status.children).toHaveLength(2);
  expect(
    fetchMock.mock.calls.some(([input]) => requestPath(input) === "/api/admin/provider-endpoints"),
  ).toBe(false);
  const request = fetchMock.mock.calls.find(([input]) =>
    requestPath(input).endsWith(`/proxies/${proxy.id}/test`),
  );
  expect(request?.[1]?.body).toBeUndefined();
});

test("renders failed probe diagnostics as only failure and latency pills", async () => {
  const proxy = customProxy();
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = requestPath(input);
    if (path.endsWith(`/proxies/${proxy.id}/test`) && init?.method === "POST") {
      return jsonResponse({
        ...proxyTestResult(proxy.id),
        reachable: false,
        status_code: null,
        latency_ms: 27,
        error_stage: "proxy_handshake",
        failure_scope: "probe_target",
      });
    }
    return jsonResponse(configuration([direct, proxy]));
  });

  renderManagement();
  fireEvent.click(await screen.findByRole("button", { name: `测试 ${proxy.name}` }));

  const status = await screen.findByRole("status", { name: /失败 · 代理握手 · 探测站点 · 27 ms/ });
  expect(within(status).getByText("失败")).toBeInTheDocument();
  expect(within(status).getByText("27 ms")).toBeInTheDocument();
  expect(screen.queryByText("代理握手")).not.toBeInTheDocument();
  expect(status.children).toHaveLength(2);
});

test("keeps request failures inside the same two pill slots", async () => {
  const proxy = customProxy();
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = requestPath(input);
    if (path.endsWith(`/proxies/${proxy.id}/test`) && init?.method === "POST") {
      return apiError(503, "proxy_test_unavailable", "代理测试服务暂不可用");
    }
    return jsonResponse(configuration([direct, proxy]));
  });

  renderManagement();
  fireEvent.click(await screen.findByRole("button", { name: `测试 ${proxy.name}` }));

  const status = await screen.findByRole("alert", { name: "代理测试服务暂不可用" });
  expect(within(status).getByText("失败")).toBeInTheDocument();
  expect(within(status).getByText("—")).toBeInTheDocument();
  expect(screen.queryByText("代理测试服务暂不可用")).not.toBeInTheDocument();
  expect(status.children).toHaveLength(2);
});

function renderManagement() {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/proxies"]}>
        <ProxyManagement />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function configuration(items: unknown[]) {
  return {
    config_revision: 1,
    global_proxy_id: direct.id,
    items,
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function apiError(status: number, code: string, message: string) {
  return new Response(JSON.stringify({ error: { code, message } }), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function customProxy() {
  return {
    id: "a81bf8f8-8fb4-45f0-926d-1cfda84884f5",
    name: "Authenticated Proxy",
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
}

function requestPath(input: RequestInfo | URL) {
  return new URL(typeof input === "string" ? input : input.toString(), "http://localhost").pathname;
}

function proxyTestResult(proxyId: string) {
  return {
    proxy_id: proxyId,
    config_revision: 1,
    proxy_config_version: 1,
    reachable: true,
    status_code: 204,
    latency_ms: 18,
    error_stage: null,
    failure_scope: null,
  };
}
