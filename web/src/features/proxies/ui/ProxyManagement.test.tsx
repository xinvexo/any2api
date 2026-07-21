import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { proxyQueryKeys } from "../model/proxy-query-keys";
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

test("renders DIRECT and explains global inheritance", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(configuration(1, [direct])));

  renderManagement();

  expect((await screen.findAllByText("DIRECT")).length).toBeGreaterThan(1);
  expect(screen.getByText(/Credential 绑定 DIRECT 时会继承此出口/)).toBeInTheDocument();
  expect(screen.getByText("尚未添加自定义代理。新代理会独立保存，不会改变当前全局出口。")).toBeInTheDocument();
});

test("creates a SOCKS5 proxy with the visible configuration revision", async () => {
  let current = configuration(1, [direct]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "POST") {
      current = configuration(2, [
        direct,
        {
          id: "a81bf8f8-8fb4-45f0-926d-1cfda84884f5",
          name: "香港出口",
          kind: "socks5",
          host: "hk.example.com",
          port: 1080,
          username: null,
          password_configured: false,
          authentication_version: 0,
          enabled: true,
          built_in: false,
          config_version: 1,
        },
      ]);
    }
    return jsonResponse(current);
  });

  renderManagement(["/proxies?editor=new"]);

  fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "香港出口" } });
  fireEvent.change(screen.getByLabelText("类型"), { target: { value: "socks5" } });
  fireEvent.change(screen.getByLabelText("主机"), { target: { value: "hk.example.com" } });
  fireEvent.change(screen.getByLabelText("端口"), { target: { value: "1080" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("hk.example.com:1080")).toBeInTheDocument();
  const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(post).toBeDefined();
  expect(JSON.parse(String(post?.[1]?.body))).toEqual({
    expected_revision: 1,
    name: "香港出口",
    kind: "socks5",
    host: "hk.example.com",
    port: 1080,
    enabled: true,
  });
});

test("does not render an editor for a DIRECT deep link", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(configuration(1, [direct])));

  renderManagement([`/proxies?editor=${direct.id}`]);

  expect(await screen.findByText("DIRECT 不可编辑")).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "编辑代理" })).not.toBeInTheDocument();
});

test("refetches the revision after a write conflict without discarding the editor", async () => {
  let getCount = 0;
  vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "POST") {
      return new Response(
        JSON.stringify({
          error: { code: "revision_conflict", message: "configuration changed" },
        }),
        { status: 409, headers: { "Content-Type": "application/json" } },
      );
    }
    getCount += 1;
    return jsonResponse(configuration(getCount === 1 ? 1 : 2, [direct]));
  });

  renderManagement(["/proxies?editor=new"]);
  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "保留的草稿" } });
  fireEvent.change(screen.getByLabelText("主机"), { target: { value: "proxy.example.com" } });
  fireEvent.change(screen.getByLabelText("端口"), { target: { value: "8080" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("2")).toBeInTheDocument();
  expect(screen.getByDisplayValue("保留的草稿")).toBeInTheDocument();
  expect(getCount).toBeGreaterThan(1);
});

test("sets proxy authentication without caching the password", async () => {
  const proxy = customProxy();
  let current = configuration(1, [direct, proxy]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = requestPath(input);
    if (path === "/api/admin/provider-endpoints") {
      return jsonResponse(providerConfiguration());
    }
    if (path.endsWith(`/proxies/${proxy.id}/authentication`) && init?.method === "PUT") {
      current = configuration(2, [direct, {
        ...proxy,
        username: "proxy-user",
        password_configured: true,
        authentication_version: 1,
        config_version: 2,
      }]);
      return jsonResponse(current);
    }
    return jsonResponse(current);
  });

  const { client } = renderManagement([`/proxies?editor=${proxy.id}`]);
  fireEvent.change(await screen.findByLabelText("用户名"), { target: { value: "proxy-user" } });
  fireEvent.change(screen.getByLabelText("密码"), { target: { value: "proxy-password" } });
  fireEvent.click(screen.getByRole("button", { name: "保存认证" }));

  expect(await screen.findByText("已为 proxy-user 配置")).toBeInTheDocument();
  expect(screen.getByLabelText("密码")).toHaveValue("");
  const request = fetchMock.mock.calls.find(([, init]) => init?.method === "PUT");
  expect(JSON.parse(String(request?.[1]?.body))).toEqual({
    expected_revision: 1,
    username: "proxy-user",
    password: "proxy-password",
  });
  expect(JSON.stringify(client.getQueryData(proxyQueryKeys.list()))).not.toContain("proxy-password");
  expect(JSON.stringify(client.getMutationCache().getAll())).not.toContain("proxy-password");
});

test("clears the password draft after a rejected authentication write", async () => {
  const proxy = customProxy();
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = requestPath(input);
    if (path === "/api/admin/provider-endpoints") {
      return jsonResponse(providerConfiguration());
    }
    if (path.endsWith(`/proxies/${proxy.id}/authentication`) && init?.method === "PUT") {
      return new Response(
        JSON.stringify({
          error: { code: "revision_conflict", message: "configuration changed" },
        }),
        { status: 409, headers: { "Content-Type": "application/json" } },
      );
    }
    return jsonResponse(configuration(1, [direct, proxy]));
  });

  renderManagement([`/proxies?editor=${proxy.id}`]);
  fireEvent.change(await screen.findByLabelText("用户名"), { target: { value: "proxy-user" } });
  const password = screen.getByLabelText("密码");
  fireEvent.change(password, { target: { value: "proxy-password" } });
  fireEvent.click(screen.getByRole("button", { name: "保存认证" }));

  await waitFor(() => expect(password).toHaveValue(""));
});

test("rejects an HTTP Basic separator and clears the local password draft", async () => {
  const proxy = customProxy();
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    if (requestPath(input) === "/api/admin/provider-endpoints") {
      return jsonResponse(providerConfiguration());
    }
    return jsonResponse(configuration(1, [direct, proxy]));
  });

  renderManagement([`/proxies?editor=${proxy.id}`]);
  fireEvent.change(await screen.findByLabelText("用户名"), { target: { value: "proxy:user" } });
  const password = screen.getByLabelText("密码");
  fireEvent.change(password, { target: { value: "proxy-password" } });
  fireEvent.click(screen.getByRole("button", { name: "保存认证" }));

  expect(await screen.findByText("用户名不能包含冒号")).toBeInTheDocument();
  expect(password).toHaveValue("");
  expect(fetchMock.mock.calls.some(([, init]) => init?.method === "PUT")).toBe(false);
});

test("tests a proxy against an existing provider endpoint", async () => {
  const proxy = customProxy();
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = requestPath(input);
    if (path === "/api/admin/provider-endpoints") {
      return jsonResponse(providerConfiguration());
    }
    if (path.endsWith(`/proxies/${proxy.id}/test`) && init?.method === "POST") {
      return jsonResponse({
        proxy_id: proxy.id,
        provider_endpoint_id: "7dd71e36-cc35-4727-903c-9555ab17290a",
        config_revision: 1,
        proxy_config_version: 1,
        provider_endpoint_config_version: 1,
        reachable: true,
        status_code: 204,
        latency_ms: 18,
        error_stage: null,
        failure_scope: null,
      });
    }
    return jsonResponse(configuration(1, [direct, proxy]));
  });

  renderManagement();
  fireEvent.click(await screen.findByRole("button", { name: `测试 ${proxy.name}` }));

  expect(await screen.findByText("可达 · HTTP 204 · 18 ms")).toBeInTheDocument();
});

test("does not offer a probe action for a disabled proxy", async () => {
  const proxy = { ...customProxy(), enabled: false };
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    if (requestPath(input) === "/api/admin/provider-endpoints") {
      return jsonResponse(providerConfiguration());
    }
    return jsonResponse(configuration(1, [direct, proxy]));
  });

  renderManagement();

  expect(await screen.findByRole("button", { name: `测试 ${proxy.name}` })).toBeDisabled();
});

function renderManagement(initialEntries = ["/proxies"]) {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return {
    client,
    ...render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries}>
        <ProxyManagement />
      </MemoryRouter>
    </QueryClientProvider>,
    ),
  };
}

function configuration(revision: number, items: unknown[]) {
  return {
    config_revision: revision,
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

function providerConfiguration() {
  return {
    config_revision: 1,
    items: [{
      id: "7dd71e36-cc35-4727-903c-9555ab17290a",
      name: "Codex",
      provider_kind: "codex",
      base_url: "https://api.openai.com/v1",
      protocol_dialect: "openai_responses",
      allow_insecure_http: false,
      allow_private_network: false,
      enabled: true,
      config_version: 1,
    }],
  };
}

function requestPath(input: RequestInfo | URL) {
  return new URL(typeof input === "string" ? input : input.toString(), "http://localhost").pathname;
}
