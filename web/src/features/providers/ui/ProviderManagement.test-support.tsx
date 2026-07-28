import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { vi } from "vitest";

import { ProviderManagement } from "./ProviderManagement";

export function renderManagement(initialEntries = ["/providers"]) {
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

export function mockAdminApis(
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

export function configuration(revision: number, items: unknown[]) {
  return { config_revision: revision, items, protocol_options: protocolOptions() };
}

export function credentialConfiguration(revision: number, items: unknown[]) {
  return {
    config_revision: revision,
    provider_endpoint_id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
    items,
  };
}

export function credential(overrides: Record<string, unknown> = {}) {
  return {
    id: "75072ca7-d922-428d-a4f8-86401567da32",
    provider_endpoint_id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
    label: "Primary Key",
    credential_kind: "api_key",
    fingerprint: "v1:0123456789abcdef",
    secret_tail: "test",
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
    requests_per_minute: null,
    enabled: true,
    secret_schema_version: 1,
    secret_version: 1,
    credential_generation: 1,
    config_version: 1,
    models: [],
    usage: usage(),
    ...overrides,
  };
}

export function endpoint(overrides: Record<string, unknown> = {}) {
  return {
    id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
    name: "Codex Primary",
    provider_kind: "codex",
    base_url: "https://api.example.com/v1",
    protocol_dialect: "openai_responses",
    upstream_protocol_dialect: null,
    enabled: true,
    config_version: 1,
    ...overrides,
  };
}

export function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function protocolOptions() {
  return [
    {
      provider_kind: "codex",
      accepted_protocol: "openai_responses",
      upstream_protocols: ["openai_responses", "openai_chat_completions"],
    },
    {
      provider_kind: "codex",
      accepted_protocol: "openai_chat_completions",
      upstream_protocols: ["openai_chat_completions"],
    },
    {
      provider_kind: "claude",
      accepted_protocol: "anthropic_messages",
      upstream_protocols: ["anthropic_messages"],
    },
    {
      provider_kind: "grok",
      accepted_protocol: "openai_responses",
      upstream_protocols: ["openai_responses", "openai_chat_completions"],
    },
    {
      provider_kind: "grok",
      accepted_protocol: "openai_chat_completions",
      upstream_protocols: ["openai_chat_completions"],
    },
  ];
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
