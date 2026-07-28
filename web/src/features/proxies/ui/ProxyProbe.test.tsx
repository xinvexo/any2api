import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { ProxyManagement } from "./ProxyManagement";
import {
  apiError,
  isProviderEndpointPath,
  isProviderEndpointRequest,
  providerConfiguration,
  providerEndpoint,
} from "./proxy-test-fixtures";

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

test("uses the first enabled Endpoint as the default proxy test target", async () => {
  const proxy = customProxy();
  const disabledEndpoint = providerEndpoint({
    id: "cf59a597-0fba-4f97-b8bb-7756dcde0c9d",
    name: "停用 Endpoint",
    enabled: false,
  });
  const enabledEndpoint = providerEndpoint({
    id: "7dd71e36-cc35-4727-903c-9555ab17290a",
    name: "默认 Endpoint",
  });
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = requestPath(input);
    if (isProviderEndpointPath(path)) {
      return jsonResponse(providerConfiguration([disabledEndpoint, enabledEndpoint]));
    }
    if (path.endsWith(`/proxies/${proxy.id}/test`) && init?.method === "POST") {
      return jsonResponse(proxyTestResult(proxy.id, enabledEndpoint.id));
    }
    return jsonResponse(configuration([direct, proxy]));
  });

  renderManagement();

  const target = await screen.findByRole("combobox", { name: "代理测试目标" });
  expect(target).toHaveValue(enabledEndpoint.id);
  const testButton = screen.getByRole("button", { name: `测试 ${proxy.name}` });
  await waitFor(() => expect(testButton).toBeEnabled());
  fireEvent.click(testButton);

  expect(await screen.findByText("可达 · HTTP 204 · 18 ms")).toBeInTheDocument();
  const request = fetchMock.mock.calls.find(([input]) =>
    requestPath(input).endsWith(`/proxies/${proxy.id}/test`),
  );
  expect(JSON.parse(String(request?.[1]?.body))).toEqual({
    provider_endpoint_id: enabledEndpoint.id,
  });
});

test("switches the test target and renders a failed probe stage inline", async () => {
  const proxy = customProxy();
  const first = providerEndpoint();
  const selected = providerEndpoint({
    id: "579a3492-5ceb-435f-91a1-aa933499f746",
    name: "Claude 内网",
    provider_kind: "claude",
    protocol_dialect: "anthropic_messages",
  });
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = requestPath(input);
    if (isProviderEndpointPath(path)) {
      return jsonResponse(providerConfiguration([first, selected]));
    }
    if (path.endsWith(`/proxies/${proxy.id}/test`) && init?.method === "POST") {
      return jsonResponse({
        ...proxyTestResult(proxy.id, selected.id),
        reachable: false,
        status_code: null,
        latency_ms: 27,
        error_stage: "proxy_handshake",
        failure_scope: "proxy",
      });
    }
    return jsonResponse(configuration([direct, proxy]));
  });

  renderManagement();

  const target = await screen.findByRole("combobox", { name: "代理测试目标" });
  fireEvent.change(target, { target: { value: selected.id } });
  fireEvent.click(screen.getByRole("button", { name: `测试 ${proxy.name}` }));

  expect(await screen.findByText("失败 · 代理握手 · 代理 · 27 ms")).toBeInTheDocument();
  const request = fetchMock.mock.calls.find(([input]) =>
    requestPath(input).endsWith(`/proxies/${proxy.id}/test`),
  );
  expect(JSON.parse(String(request?.[1]?.body))).toEqual({
    provider_endpoint_id: selected.id,
  });
});

test("disables proxy tests when no Provider Endpoint exists", async () => {
  const proxy = customProxy();
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) =>
    jsonResponse(
      isProviderEndpointRequest(input)
        ? providerConfiguration([])
        : configuration([direct, proxy]),
    ),
  );

  renderManagement();

  expect(await screen.findByText("暂无 Provider Endpoint，代理连通性测试不可用。")).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "代理测试目标" })).toBeDisabled();
  expect(screen.getByRole("button", { name: `测试 ${proxy.name}` })).toBeDisabled();
});

test("shows Endpoint loading failures and keeps proxy tests disabled", async () => {
  const proxy = customProxy();
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    if (isProviderEndpointRequest(input)) {
      return apiError(503, "provider_endpoint_unavailable", "Endpoint 列表暂不可用");
    }
    return jsonResponse(configuration([direct, proxy]));
  });

  renderManagement();

  expect(await screen.findByText("测试目标加载失败：Endpoint 列表暂不可用")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: `测试 ${proxy.name}` })).toBeDisabled();
});

test("shows a proxy test request error in the affected row", async () => {
  const proxy = customProxy();
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = requestPath(input);
    if (isProviderEndpointPath(path)) {
      return jsonResponse(providerConfiguration());
    }
    if (path.endsWith(`/proxies/${proxy.id}/test`) && init?.method === "POST") {
      return apiError(503, "proxy_test_unavailable", "代理测试服务暂不可用");
    }
    return jsonResponse(configuration([direct, proxy]));
  });

  renderManagement();

  const testButton = await screen.findByRole("button", { name: `测试 ${proxy.name}` });
  await waitFor(() => expect(testButton).toBeEnabled());
  fireEvent.click(testButton);

  expect(await screen.findByText("代理测试服务暂不可用")).toBeInTheDocument();
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

function proxyTestResult(proxyId: string, providerEndpointId: string) {
  return {
    proxy_id: proxyId,
    provider_endpoint_id: providerEndpointId,
    config_revision: 1,
    proxy_config_version: 1,
    provider_endpoint_config_version: 1,
    reachable: true,
    status_code: 204,
    latency_ms: 18,
    error_stage: null,
    failure_scope: null,
  };
}
