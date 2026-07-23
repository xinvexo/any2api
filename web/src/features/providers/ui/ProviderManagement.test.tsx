import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { ProviderManagement } from "./ProviderManagement";

afterEach(() => vi.restoreAllMocks());

test("shows the empty Provider state", async () => {
  mockAdminApis(() => configuration(1, []));

  renderManagement();

  expect(await screen.findByText("还没有 Codex Endpoint")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "新增" })).toBeInTheDocument();
});

test("expands endpoint accordion to show nested API keys on the same page", async () => {
  const fetchMock = mockAdminApis(
    () => configuration(1, [endpoint()]),
    () => credentialConfiguration(3, [credential()]),
  );

  renderManagement();

  const header = await screen.findByRole("button", { name: "展开 Codex Primary 的 API Key" });
  expect(header).toHaveAttribute("aria-expanded", "false");
  expect(screen.queryByText("Primary Key")).not.toBeInTheDocument();
  expect(screen.queryByRole("link", { name: /API Key/ })).not.toBeInTheDocument();

  fireEvent.click(header);

  expect(await screen.findByRole("button", { name: "收起 Codex Primary 的 API Key" })).toHaveAttribute("aria-expanded", "true");
  expect(await screen.findByText("Primary Key")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "配置 Primary Key 的模型" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "编辑 Primary Key" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "删除 Primary Key" })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "测试 Primary Key" })).not.toBeInTheDocument();
  expect(fetchMock.mock.calls.map(([input]) => String(input))).toEqual(
    expect.arrayContaining([
      "/api/admin/provider-endpoints",
      expect.stringContaining("/credentials"),
      "/api/admin/proxies",
    ]),
  );
});

test("creates a Claude private HTTP endpoint directly from the Base URL", async () => {
  let current = configuration(1, []);
  const fetchMock = mockAdminApis(
    () => current,
    () => credentialConfiguration(1, []),
    (input, init) => {
      if (String(input).includes("/provider-endpoints") && init?.method === "POST") {
        current = configuration(2, [
          endpoint({
            name: "本地 Claude",
            provider_kind: "claude",
            base_url: "http://127.0.0.1:8080",
            protocol_dialect: "anthropic_messages",
          }),
        ]);
        return jsonResponse(current);
      }
      return null;
    },
  );

  renderManagement(["/providers?kind=claude&editor=new"]);

  fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "本地 Claude" } });
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: "http://127.0.0.1:8080" } });
  expect(screen.queryByRole("switch", { name: "允许普通 HTTP" })).not.toBeInTheDocument();
  expect(screen.queryByRole("switch", { name: "允许内网地址" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("http://127.0.0.1:8080")).toBeInTheDocument();
  const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(JSON.parse(String(post?.[1]?.body))).toEqual({
    expected_revision: 1,
    name: "本地 Claude",
    provider_kind: "claude",
    base_url: "http://127.0.0.1:8080",
    protocol_dialect: "anthropic_messages",
    enabled: true,
  });
});

test("refetches after a revision conflict without discarding the endpoint draft", async () => {
  let getCount = 0;
  mockAdminApis(
    () => {
      getCount += 1;
      return configuration(getCount === 1 ? 1 : 2, []);
    },
    () => credentialConfiguration(1, []),
    (_input, init) => {
      if (init?.method === "POST") {
        return new Response(
          JSON.stringify({ error: { code: "revision_conflict", message: "configuration changed" } }),
          { status: 409, headers: { "Content-Type": "application/json" } },
        );
      }
      return null;
    },
  );

  renderManagement(["/providers?editor=new"]);
  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "保留的 Endpoint 草稿" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(screen.getByDisplayValue("保留的 Endpoint 草稿")).toBeInTheDocument();
  expect(await screen.findByText(/配置已发生变化/)).toBeInTheDocument();
  expect(getCount).toBeGreaterThan(1);
});

test("preserves the draft but blocks overwrite when the endpoint version changed", async () => {
  let getCount = 0;
  const fetchMock = mockAdminApis(
    () => {
      getCount += 1;
      return configuration(
        getCount === 1 ? 1 : 2,
        [
          endpoint({
            name: getCount === 1 ? "Codex Primary" : "Codex Renamed Elsewhere",
            config_version: getCount === 1 ? 1 : 2,
          }),
        ],
      );
    },
    () => credentialConfiguration(1, []),
    (_input, init) => {
      if (init?.method === "PATCH") {
        return new Response(
          JSON.stringify({
            error: { code: "revision_conflict", message: "configuration changed" },
          }),
          { status: 409, headers: { "Content-Type": "application/json" } },
        );
      }
      return null;
    },
  );

  renderManagement(["/providers?editor=1e96eff2-7b3f-4974-b013-8fd2f44c8c1f"]);
  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "Local Draft" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(screen.getByDisplayValue("Local Draft")).toBeInTheDocument();
  expect(await screen.findByText(/已被其他操作修改/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  const patches = fetchMock.mock.calls.filter(([, init]) => init?.method === "PATCH");
  expect(JSON.parse(String(patches[0]?.[1]?.body))).toMatchObject({
    expected_revision: 1,
    expected_config_version: 1,
  });
  expect(patches).toHaveLength(1);
});

test("preserves the draft and blocks saving when the endpoint was deleted", async () => {
  let getCount = 0;
  mockAdminApis(
    () => {
      getCount += 1;
      return configuration(getCount === 1 ? 1 : 2, getCount === 1 ? [endpoint()] : []);
    },
    () => credentialConfiguration(1, []),
    (_input, init) => {
      if (init?.method === "PATCH") {
        return new Response(
          JSON.stringify({
            error: { code: "revision_conflict", message: "configuration changed" },
          }),
          { status: 409, headers: { "Content-Type": "application/json" } },
        );
      }
      return null;
    },
  );

  renderManagement(["/providers?editor=1e96eff2-7b3f-4974-b013-8fd2f44c8c1f"]);
  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "Retained Draft" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText(/已从最新配置中删除/)).toBeInTheDocument();
  expect(screen.getByDisplayValue("Retained Draft")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
});

function renderManagement(initialEntries = ["/providers"]) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries}>
        <ProviderManagement />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function mockAdminApis(
  endpoints: () => unknown,
  credentials: () => unknown = () => credentialConfiguration(1, []),
  override?: (input: RequestInfo | URL, init?: RequestInit) => Response | null,
) {
  return vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    if (override) {
      const custom = override(input, init);
      if (custom) {
        return custom;
      }
    }
    const path = String(input);
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (path.includes("/credentials")) {
      return jsonResponse(credentials());
    }
    return jsonResponse(endpoints());
  });
}

function configuration(revision: number, items: unknown[]) {
  return { config_revision: revision, items };
}

function credentialConfiguration(revision: number, items: unknown[]) {
  return {
    config_revision: revision,
    provider_endpoint_id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
    items,
  };
}

function credential(overrides: Record<string, unknown> = {}) {
  return {
    id: "75072ca7-d922-428d-a4f8-86401567da32",
    provider_endpoint_id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
    label: "Primary Key",
    credential_kind: "api_key",
    fingerprint: "v1:0123456789abcdef",
    secret_tail: "test",
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
    max_concurrency: 4,
    enabled: true,
    secret_schema_version: 1,
    secret_version: 1,
    credential_generation: 1,
    config_version: 1,
    models: [],
    ...overrides,
  };
}

function proxyConfiguration() {
  return {
    config_revision: 2,
    global_proxy_id: "f0335fed-e5a9-4081-966b-37efe4a109a8",
    items: [
      {
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
      },
      {
        id: "f0335fed-e5a9-4081-966b-37efe4a109a8",
        name: "香港代理",
        kind: "http",
        host: "proxy.example.com",
        port: 8080,
        username: null,
        password_configured: false,
        authentication_version: 0,
        enabled: true,
        built_in: false,
        config_version: 1,
      },
    ],
  };
}

function endpoint(overrides: Record<string, unknown> = {}) {
  return {
    id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
    name: "Codex Primary",
    provider_kind: "codex",
    base_url: "https://api.example.com/v1",
    protocol_dialect: "openai_responses",
    enabled: true,
    config_version: 1,
    ...overrides,
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
