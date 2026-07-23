import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { GatewayApiKeyManagement } from "./GatewayApiKeyManagement";

afterEach(() => vi.restoreAllMocks());

test("creates a gateway key and keeps the plaintext available in the table for this session", async () => {
  const token = `a2k_v1_${"b".repeat(43)}`;
  let configuration: Record<string, unknown> = { config_revision: 1, items: [] };
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "POST") {
      configuration = {
        config_revision: 2,
        items: [
          {
            id: "key-1",
            name: "Desktop",
            token,
            token_prefix: token.slice(0, 16),
            token_version: 1,
            config_version: 1,
            enabled: true,
            revoked_at: null,
            created_at: "2026-07-19 10:00:00",
            last_used_at: null,
            usage: usage(),
          },
        ],
        token,
      };
      return jsonResponse(configuration);
    }
    return jsonResponse(configuration);
  });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/keys?editor=new"]}>
        <GatewayApiKeyManagement />
        <LocationProbe />
      </MemoryRouter>
    </QueryClientProvider>,
  );

  fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "Desktop" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => {
    expect(screen.queryByLabelText("名称")).not.toBeInTheDocument();
  });
  expect(await screen.findByText("Desktop")).toBeInTheDocument();
  expect(screen.queryByLabelText("本次生成的网关密钥")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "关闭密钥回执" })).not.toBeInTheDocument();

  expect(screen.getByText(token)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "隐藏 Desktop 的密钥" }));
  expect(screen.queryByText(token)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "显示 Desktop 的密钥" }));
  expect(screen.getByText(token)).toBeInTheDocument();

  expect(fetchMock.mock.calls.some(([, init]) => init?.method === "POST")).toBe(true);
  expect(screen.getByTestId("location")).not.toHaveTextContent(token);
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
          usage: usage({
            total_requests: 177,
            successful_requests: 134,
            failed_requests: 43,
            recent_outcomes: [
              { status_code: 200 },
              { status_code: 429 },
              { status_code: 204 },
              { status_code: 503 },
            ],
          }),
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
  expect(screen.getByText("成功: 134")).toBeInTheDocument();
  expect(screen.getByText("失败: 43")).toBeInTheDocument();
  expect(screen.getByText("75.7%")).toBeInTheDocument();
  expect(screen.getByRole("img", { name: /Desktop 最近 4 次调用/ })).toBeInTheDocument();
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
        usage: usage(),
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
            usage: usage(),
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

function usage(overrides: Record<string, unknown> = {}) {
  return {
    total_requests: 0,
    successful_requests: 0,
    failed_requests: 0,
    recent_outcomes: [],
    ...overrides,
  };
}
