import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { CodexRateCardManagement } from "./CodexRateCardManagement";

afterEach(() => vi.restoreAllMocks());

test("edits the structured card and sends a new hidden ID", async () => {
  let current = configuration(1, 25);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      current = configuration(2, 30);
    }
    return new Response(JSON.stringify(current), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  });
  renderPage();

  const exchange = await screen.findByLabelText("Credits / USD");
  expect(screen.queryByRole("heading", { name: "Codex 额度费率" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "刷新额度费率" }).closest("header"))
    .toHaveClass("min-h-8", "border-b", "pb-3");
  fireEvent.change(exchange, { target: { value: "30" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => {
    const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
    expect(patch).toBeDefined();
    const body = JSON.parse(String(patch?.[1]?.body));
    expect(body.expected_revision).toBe(1);
    expect(body.updates).toHaveLength(1);
    expect(body.updates[0].key).toBe("oauth.codex.rate_card");
    expect(body.updates[0].value.credits_per_usd).toBe(30);
    expect(body.updates[0].value.id).toMatch(/^codex_rate_card_/u);
    expect(body.updates[0].value.id).not.toBe("openai_codex_credits_2026_08_11");
  });
});

test("uses a model selector backed by setting options", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify(configuration(1, 25)), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  }));
  renderPage();

  const model = await screen.findByRole("combobox", { name: "模型名称" });
  expect(model).toHaveAttribute("data-value", "gpt-5.6-sol");
  fireEvent.click(model);
  expect(screen.getByRole("option", { name: "gpt-5.6-terra" })).toBeInTheDocument();
  expect(screen.queryByRole("textbox", { name: "模型名称" })).not.toBeInTheDocument();
});

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const router = createMemoryRouter([
    { path: "/quota-rates", element: <CodexRateCardManagement /> },
  ], { initialEntries: ["/quota-rates"] });
  return render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
}

function configuration(revision: number, creditsPerUsd: number) {
  const card = {
    id: "openai_codex_credits_2026_08_11",
    credits_per_usd: creditsPerUsd,
    models: {
      "gpt-5.6-sol": {
        standard: {
          input_nanos_per_million: 125_000_000_000,
          cached_input_nanos_per_million: 12_500_000_000,
          output_nanos_per_million: 750_000_000_000,
        },
      },
    },
  };
  return {
    config_revision: revision,
    items: [{
      key: "oauth.codex.rate_card",
      value_type: "codex_rate_card",
      default_value: card,
      override_value: null,
      effective_value: card,
      min_value: null,
      max_value: null,
      allowed_values: null,
      options: ["gpt-5.6-sol", "gpt-5.6-terra"],
      apply_mode: "hot_reload",
      web_group: "额度费率",
      description: "Codex rate card",
    }],
  };
}
