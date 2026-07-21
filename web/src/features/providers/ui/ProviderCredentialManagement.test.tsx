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
    if (init?.method === "POST") {
      credentials = credentialConfiguration(3, [credential()]);
      return jsonResponse(credentials);
    }
    return jsonResponse(credentials);
  });
  const { client } = renderManagement([`/providers/${endpoint.id}?credential=new`]);

  expect(await screen.findByRole("option", { name: "DIRECT（继承全局：香港代理）" })).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "Primary Key" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: secret } });
  fireEvent.change(screen.getByLabelText("最大并发"), { target: { value: "8" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByLabelText("本次保存的 API Key")).toHaveValue(secret);
  const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(JSON.parse(String(post?.[1]?.body))).toMatchObject({
    api_key: secret,
    max_concurrency: 8,
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
  });
  expect(screen.getByTestId("location")).not.toHaveTextContent(secret);
  expect(JSON.stringify(client.getQueryCache().getAll().map((query) => query.state.data))).not.toContain(secret);
  expect(JSON.stringify(client.getMutationCache().getAll())).not.toContain(secret);

  fireEvent.click(screen.getByRole("button", { name: "关闭回执" }));
  await waitFor(() => expect(screen.queryByLabelText("本次保存的 API Key")).not.toBeInTheDocument());
  expect(document.body.innerHTML).not.toContain(secret);
  expect(screen.getByTestId("location")).not.toHaveTextContent(secret);
});

test("metadata updates never send the API Key field", async () => {
  let credentials = credentialConfiguration(3, [credential()]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    if (String(input) === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (init?.method === "PATCH") {
      credentials = credentialConfiguration(4, [
        credential({ label: "Edited", max_concurrency: 12, config_version: 2 }),
      ]);
    }
    return jsonResponse(credentials);
  });
  renderManagement([`/providers/${endpoint.id}?credential=${credentialId}`]);

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

function renderManagement(initialEntries: string[]) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const result = render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries}>
        <ProviderCredentialManagement endpoint={endpoint} />
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
  allowInsecureHttp: false,
  allowPrivateNetwork: false,
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
    ...overrides,
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
