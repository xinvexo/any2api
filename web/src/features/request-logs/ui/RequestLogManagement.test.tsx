import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { RequestLogManagement } from "./RequestLogManagement";

afterEach(() => vi.restoreAllMocks());

test("renders recent request metadata without prompt or secret content", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    new Response(
      JSON.stringify({
        items: [
          {
            request_id: "11111111-1111-4111-8111-111111111111",
            started_at_ms: 1_700_000_000_000,
            config_revision: 9,
            gateway_api_key_id: "22222222-2222-4222-8222-222222222222",
            ingress_protocol: "openai_responses",
            operation: "responses",
            public_model: "codex-local",
            provider_endpoint_id: "33333333-3333-4333-8333-333333333333",
            credential_id: "44444444-4444-4444-8444-444444444444",
            proxy_profile_id: "00000000-0000-0000-0000-000000000000",
            status_code: 200,
            error_class: null,
            attempt_count: 1,
            latency_ms: 30,
            first_token_ms: null,
            input_tokens: null,
            output_tokens: null,
            cache_read_tokens: null,
            cache_write_tokens: null,
            is_stream: false,
          },
        ],
        telemetry: {
          queued_records: 0,
          dropped_records: 2,
          persisted_records: 8,
        },
      }),
      { status: 200, headers: { "Content-Type": "application/json" } },
    ),
  );

  renderManagement();

  expect(await screen.findByText("codex-local")).toBeInTheDocument();
  expect(screen.getByText("11111111-1111-4111-8111-111111111111")).toBeInTheDocument();
  expect(screen.getByText(/丢弃 2/)).toBeInTheDocument();
  expect(screen.queryByText("private prompt")).not.toBeInTheDocument();
  expect(screen.queryByText("provider-secret")).not.toBeInTheDocument();
});

function renderManagement() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <MemoryRouter>
      <QueryClientProvider client={client}>
        <RequestLogManagement />
      </QueryClientProvider>
    </MemoryRouter>,
  );
}
