export function apiError(status: number, code: string, message: string) {
  return new Response(JSON.stringify({ error: { code, message } }), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

export function isProviderEndpointRequest(input: RequestInfo | URL) {
  return isProviderEndpointPath(requestPath(input));
}

export function isProviderEndpointPath(path: string) {
  return path === "/api/admin/provider-endpoints";
}

export function providerConfiguration(items = [providerEndpoint()]) {
  return {
    config_revision: 1,
    items,
    protocol_options: [
      {
        provider_kind: "codex",
        accepted_protocol: "openai_responses",
        upstream_protocols: ["openai_responses"],
      },
      {
        provider_kind: "claude",
        accepted_protocol: "anthropic_messages",
        upstream_protocols: ["anthropic_messages"],
      },
    ],
  };
}

export function providerEndpoint(overrides: Record<string, unknown> = {}) {
  return {
    id: "7dd71e36-cc35-4727-903c-9555ab17290a",
    name: "Codex",
    provider_kind: "codex",
    base_url: "https://api.openai.com/v1",
    protocol_dialect: "openai_responses",
    upstream_protocol_dialect: null,
    enabled: true,
    config_version: 1,
    ...overrides,
  };
}

function requestPath(input: RequestInfo | URL) {
  return new URL(typeof input === "string" ? input : input.toString(), "http://localhost").pathname;
}
