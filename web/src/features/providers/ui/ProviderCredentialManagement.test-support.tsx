import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { ReactNode } from "react";

import type { ProviderEndpoint } from "../api/provider-contracts";
import { ProviderCredentialManagement } from "./ProviderCredentialManagement";

export function renderCredentialManagement(initialEntries: string[], children?: ReactNode) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const result = render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries}>
        <ProviderCredentialManagement endpoint={endpoint} embedded />
        {children}
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return { ...result, client };
}

export const endpoint: ProviderEndpoint = {
  id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
  name: "Codex Primary",
  providerKind: "codex",
  baseUrl: "https://api.example.com",
  protocolDialect: "openai_responses",
  upstreamProtocolDialect: null,
  enabled: true,
  configVersion: 1,
};

export const credentialId = "75072ca7-d922-428d-a4f8-86401567da32";

export function credential(overrides: Record<string, unknown> = {}) {
  return {
    id: credentialId,
    provider_endpoint_id: endpoint.id,
    label: "Primary Key",
    credential_kind: "api_key",
    fingerprint: "v2:0123456789abcdef",
    secret_tail: "test",
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
    requests_per_minute: 4,
    enabled: true,
    secret_version: 1,
    credential_generation: 1,
    config_version: 1,
    models: [],
    usage: usage(),
    ...overrides,
  };
}

export function credentialTestResult() {
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

export function credentialConfiguration(revision: number, items: unknown[]) {
  return { config_revision: revision, provider_endpoint_id: endpoint.id, items };
}

export function proxyConfiguration() {
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

export function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function usage() {
  const windowMs = 2 * 60 * 1000;
  const newest = Math.floor(Date.now() / windowMs) * windowMs;
  return {
    total_requests: 3,
    successful_requests: 2,
    failed_requests: 1,
    window_minutes: 2,
    window_slots: Array.from({ length: 30 }, (_, index) => ({
      started_at_ms: newest - (29 - index) * windowMs,
      total_requests: index >= 27 ? 1 : 0,
      successful_requests: index === 27 || index === 29 ? 1 : 0,
      failed_requests: index === 28 ? 1 : 0,
    })),
  };
}
