import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { GatewayApiKeyManagement } from "./GatewayApiKeyManagement";

afterEach(() => {
  vi.restoreAllMocks();
});

test("shows plaintext tokens from the list response", async () => {
  const token = `a2k_v1_${"b".repeat(43)}`;
  const tokenPrefix = token.slice(0, 16);
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    jsonResponse({
      config_revision: 2,
      items: [
        {
          id: "key-1",
          name: "Desktop",
          token,
          token_prefix: tokenPrefix,
          token_version: 1,
          config_version: 1,
          enabled: true,
          revoked_at: null,
          created_at: "2026-07-19 10:00:00",
          last_used_at: null,
        },
      ],
    }),
  );

  render(
    <QueryClientProvider
      client={
        new QueryClient({
          defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
        })
      }
    >
      <MemoryRouter initialEntries={["/keys"]}>
        <GatewayApiKeyManagement />
      </MemoryRouter>
    </QueryClientProvider>,
  );

  expect(await screen.findByRole("caption", { name: "网关密钥列表" })).toBeInTheDocument();
  expect(screen.getByText("Desktop")).toBeInTheDocument();
  expect(screen.getByText(token)).toBeInTheDocument();
  expect(screen.queryByText("创建后仅展示一次")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /轮换/ })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /撤销/ })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "删除 Desktop" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "新增密钥" })).toBeInTheDocument();
});

test("regenerates the key from the edit drawer and refreshes plaintext in the table", async () => {
  const oldToken = `a2k_v1_${"c".repeat(43)}`;
  const newToken = `a2k_v1_${"d".repeat(43)}`;
  let configuration: Record<string, unknown> = {
    config_revision: 3,
    items: [
      {
        id: "key-1",
        name: "Desktop",
        token: oldToken,
        token_prefix: oldToken.slice(0, 16),
        token_version: 1,
        config_version: 1,
        enabled: true,
        revoked_at: null,
        created_at: "2026-07-19 10:00:00",
        last_used_at: null,
      },
    ],
  };
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const url = String(input);
    if (init?.method === "POST" && url.includes("/rotate")) {
      configuration = {
        config_revision: 4,
        items: [
          {
            id: "key-1",
            name: "Desktop",
            token: newToken,
            token_prefix: newToken.slice(0, 16),
            token_version: 2,
            config_version: 2,
            enabled: true,
            revoked_at: null,
            created_at: "2026-07-19 10:00:00",
            last_used_at: null,
          },
        ],
        token: newToken,
      };
      return jsonResponse(configuration);
    }
    return jsonResponse(configuration);
  });

  render(
    <QueryClientProvider
      client={
        new QueryClient({
          defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
        })
      }
    >
      <MemoryRouter initialEntries={["/keys?editor=key-1"]}>
        <GatewayApiKeyManagement />
      </MemoryRouter>
    </QueryClientProvider>,
  );

  expect(await screen.findByLabelText("名称")).toHaveValue("Desktop");
  fireEvent.click(screen.getByLabelText("重新生成密钥"));
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => {
    expect(screen.queryByLabelText("名称")).not.toBeInTheDocument();
  });
  expect(await screen.findByText(newToken)).toBeInTheDocument();
  expect(
    fetchMock.mock.calls.some(
      ([input, init]) => init?.method === "POST" && String(input).includes("/rotate"),
    ),
  ).toBe(true);
});

function LocationProbe() {
  const location = useLocation();
  return <span data-testid="location" hidden>{`${location.pathname}${location.search}`}</span>;
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

// Keep probe import usage quiet if unused in current tests.
void LocationProbe;
