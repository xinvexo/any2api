import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import type { ProviderEndpoint } from "../api/provider-contracts";
import { ProviderCredentialManagement } from "./ProviderCredentialManagement";

afterEach(() => vi.restoreAllMocks());

test("creates a credential without retaining its secret in application caches", async () => {
  const secret = "sk-browser-secret-value";
  let credentials = credentialConfiguration(2, []);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (path.endsWith(`/provider-credentials/${credentialId}/test`)) {
      return jsonResponse(credentialTestResult());
    }
    if (path.endsWith(`/provider-credentials/${credentialId}/models`) && init?.method === "PUT") {
      credentials = credentialConfiguration(4, [
        credential({ config_version: 2, models: ["gpt-5.1-codex"] }),
      ]);
      return jsonResponse(credentials);
    }
    if (path.endsWith(`/provider-endpoints/${endpoint.id}/credentials`) && init?.method === "POST") {
      credentials = credentialConfiguration(3, [credential()]);
      return jsonResponse(credentials);
    }
    return jsonResponse(credentials);
  });
  const { client } = renderManagement([`/providers/codex?keys=${endpoint.id}&credential=new`]);

  expect(await screen.findByRole("option", { name: "DIRECT" })).toBeInTheDocument();
  expect(screen.getByRole("option", { name: "香港代理" })).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "Primary Key" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: secret } });
  fireEvent.change(screen.getByLabelText("最大并发"), { target: { value: "8" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  const model = await screen.findByRole("checkbox", { name: "gpt-5.1-codex" });
  const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(JSON.parse(String(post?.[1]?.body))).toMatchObject({
    api_key: secret,
    max_concurrency: 8,
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
  });
  expect(screen.queryByLabelText("本次保存的 API Key")).not.toBeInTheDocument();
  expect(screen.getByTestId("location")).not.toHaveTextContent(secret);
  expect(JSON.stringify(client.getQueryCache().getAll().map((query) => query.state.data))).not.toContain(secret);
  expect(JSON.stringify(client.getMutationCache().getAll())).not.toContain(secret);

  fireEvent.click(model);
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(screen.getByTestId("location")).toHaveTextContent("/providers"));
  const modelPut = fetchMock.mock.calls.find(
    ([input, init]) => String(input).endsWith(`/provider-credentials/${credentialId}/models`) && init?.method === "PUT",
  );
  expect(JSON.parse(String(modelPut?.[1]?.body))).toEqual({
    expected_revision: 3,
    expected_config_version: 1,
    models: ["gpt-5.1-codex"],
  });
  expect(document.body.innerHTML).not.toContain(secret);
  expect(screen.getByTestId("location")).not.toHaveTextContent(secret);
});

test("edits credential metadata without sending the secret", async () => {
  let credentials = credentialConfiguration(3, [credential()]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (init?.method === "PATCH") {
      credentials = credentialConfiguration(4, [
        credential({ label: "Edited", max_concurrency: 12, config_version: 2 }),
      ]);
    }
    return jsonResponse(credentials);
  });
  renderManagement([`/providers/codex?keys=${endpoint.id}&credential=${credentialId}`]);

  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "Edited" } });
  fireEvent.change(screen.getByLabelText("最大并发"), { target: { value: "12" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await screen.findByText("Edited");
  const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
  const body = JSON.parse(String(patch?.[1]?.body)) as Record<string, unknown>;
  expect(body).toMatchObject({
    expected_revision: 3,
    expected_config_version: 1,
    label: "Edited",
    max_concurrency: 12,
  });
  expect(body).not.toHaveProperty("api_key");
});

test("opens a credential model picker and loads the current upstream catalog", async () => {
  const credentials = credentialConfiguration(3, [credential()]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const path = String(input);
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (path.endsWith(`/provider-credentials/${credentialId}/test`)) {
      return jsonResponse({
        config_revision: 3,
        provider_endpoint_config_version: 1,
        credential_config_version: 1,
        credential_generation: 1,
        secret_version: 1,
        proxy_config_version: 1,
        credential_id: credentialId,
        provider_endpoint_id: endpoint.id,
        proxy_id: "f0335fed-e5a9-4081-966b-37efe4a109a8",
        reachable: true,
        accepted: true,
        catalog_valid: true,
        status_code: 200,
        latency_ms: 18,
        auth_error_cleared: true,
        error_stage: null,
        failure_scope: null,
        models: ["gpt-5.1-codex"],
      });
    }
    return jsonResponse(credentials);
  });
  renderManagement([`/providers/codex?keys=${endpoint.id}`]);

  fireEvent.click(await screen.findByRole("button", { name: "配置 Primary Key 的模型" }));

  expect(await screen.findByRole("checkbox", { name: "gpt-5.1-codex" })).toBeInTheDocument();
  expect(screen.getByText(/已读取 1 个模型/)).toBeInTheDocument();
  const request = fetchMock.mock.calls.find(([input]) => String(input).endsWith("/test"));
  expect(request?.[1]?.method).toBe("POST");
  expect(request?.[1]?.body).toBeUndefined();
});

function renderManagement(initialEntries: string[]) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const result = render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries}>
        <ProviderCredentialManagement endpoint={endpoint} embedded />
        <LocationProbe />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return { ...result, client };
}

function LocationProbe() {
  const location = useLocation();
  return <span data-testid="location" hidden>{`${location.pathname}${location.search}`}</span>;
}

const endpoint: ProviderEndpoint = {
  id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
  name: "Codex Primary",
  providerKind: "codex",
  baseUrl: "https://api.example.com",
  protocolDialect: "openai_responses",
  upstreamProtocolDialect: null,
  enabled: true,
  configVersion: 1,
};

const credentialId = "75072ca7-d922-428d-a4f8-86401567da32";

function credential(overrides: Record<string, unknown> = {}) {
  return {
    id: credentialId,
    provider_endpoint_id: endpoint.id,
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

function credentialTestResult() {
  return {
    config_revision: 3,
    provider_endpoint_config_version: 1,
    credential_config_version: 1,
    credential_generation: 1,
    secret_version: 1,
    proxy_config_version: 1,
    credential_id: credentialId,
    provider_endpoint_id: endpoint.id,
    proxy_id: "00000000-0000-0000-0000-000000000000",
    reachable: true,
    accepted: true,
    catalog_valid: true,
    status_code: 200,
    latency_ms: 18,
    auth_error_cleared: true,
    error_stage: null,
    failure_scope: null,
    models: ["gpt-5.1-codex", "gpt-5.1-codex-mini"],
  };
}

function credentialConfiguration(revision: number, items: unknown[]) {
  return { config_revision: revision, provider_endpoint_id: endpoint.id, items };
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

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
