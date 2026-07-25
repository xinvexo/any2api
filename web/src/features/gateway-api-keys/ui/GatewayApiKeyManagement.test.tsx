import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { GatewayApiKeyManagement } from "./GatewayApiKeyManagement";

afterEach(() => vi.restoreAllMocks());

const tokenA = `sk-${"b".repeat(48)}`;
const tokenB = `sk-${"c".repeat(48)}`;
const tokenC = `sk-${"d".repeat(48)}`;

test("creates a gateway key with the form token and exposes copy in row actions", async () => {
  let configuration: Record<string, unknown> = { config_revision: 1, items: [] };
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "POST") {
      const body = JSON.parse(String(init.body)) as { token: string; name: string };
      configuration = {
        config_revision: 2,
        items: [
          {
            id: "key-1",
            name: body.name,
            token: body.token,
            token_prefix: body.token.slice(0, 16),
            token_version: 1,
            config_version: 1,
            enabled: true,
            revoked_at: null,
            created_at: "2026-07-19 10:00:00",
            last_used_at: null,
            usage: usage(),
          },
        ],
        token: body.token,
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
  const tokenInput = screen.getByLabelText("密钥") as HTMLInputElement;
  expect(tokenInput.value).toMatch(/^sk-[A-Za-z0-9]{48}$/);
  fireEvent.change(tokenInput, { target: { value: tokenA } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => {
    expect(screen.queryByLabelText("名称")).not.toBeInTheDocument();
  });
  expect(await screen.findByText("Desktop")).toBeInTheDocument();
  expect(screen.queryByText(tokenA)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "复制 Desktop 的密钥" })).toBeInTheDocument();

  const createCall = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(JSON.parse(String(createCall?.[1]?.body))).toMatchObject({
    name: "Desktop",
    token: tokenA,
    enabled: true,
  });
  expect(screen.getByTestId("location")).not.toHaveTextContent(tokenA);
});

test("lists keys without showing plaintext tokens", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    jsonResponse({
      config_revision: 2,
      items: [
        {
          id: "key-1",
          name: "Desktop",
          token: tokenA,
          token_prefix: tokenA.slice(0, 16),
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
  expect(screen.queryByText(tokenA)).not.toBeInTheDocument();
  expect(screen.queryByRole("columnheader", { name: "密钥" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "复制 Desktop 的密钥" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "禁用 Desktop" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "删除 Desktop" })).toBeInTheDocument();
  expect(screen.getByText("成功 134")).toBeInTheDocument();
  expect(screen.getByText("失败 43")).toBeInTheDocument();
  expect(screen.queryByText("暂无调用")).not.toBeInTheDocument();
});

test("toggles enabled from the row action without opening the editor", async () => {
  let configuration: Record<string, unknown> = {
    config_revision: 2,
    items: [
      {
        id: "key-1",
        name: "Desktop",
        token: tokenA,
        token_prefix: tokenA.slice(0, 16),
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
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      const body = JSON.parse(String(init.body)) as { enabled: boolean };
      configuration = {
        config_revision: 3,
        items: [
          {
            ...(configuration.items as Record<string, unknown>[])[0],
            enabled: body.enabled,
            config_version: 2,
          },
        ],
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
      <MemoryRouter initialEntries={["/keys"]}>
        <GatewayApiKeyManagement />
      </MemoryRouter>
    </QueryClientProvider>,
  );

  expect(await screen.findByText("已启用")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "禁用 Desktop" }));

  await waitFor(() => {
    expect(screen.getByText("已停用")).toBeInTheDocument();
  });
  expect(screen.getByRole("button", { name: "启用 Desktop" })).toBeInTheDocument();
  const patchCall = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
  expect(JSON.parse(String(patchCall?.[1]?.body))).toMatchObject({
    name: "Desktop",
    enabled: false,
  });
});

test("edit drawer echoes the key and rotate uses the generated value", async () => {
  let configuration: Record<string, unknown> = {
    config_revision: 3,
    items: [
      {
        id: "key-1",
        name: "Desktop",
        token: tokenB,
        token_prefix: tokenB.slice(0, 16),
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
      const body = JSON.parse(String(init.body)) as { token: string };
      configuration = {
        config_revision: 4,
        items: [
          {
            id: "key-1",
            name: "Desktop",
            token: body.token,
            token_prefix: body.token.slice(0, 16),
            token_version: 2,
            config_version: 2,
            enabled: true,
            revoked_at: null,
            created_at: "2026-07-19 10:00:00",
            last_used_at: null,
            usage: usage(),
          },
        ],
        token: body.token,
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
  expect(screen.getByLabelText("密钥")).toHaveValue(tokenB);
  fireEvent.click(screen.getByRole("button", { name: "生成" }));
  const generated = (screen.getByLabelText("密钥") as HTMLInputElement).value;
  expect(generated).toMatch(/^sk-[A-Za-z0-9]{48}$/);
  expect(generated).not.toBe(tokenB);
  fireEvent.change(screen.getByLabelText("密钥"), { target: { value: tokenC } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => {
    expect(screen.queryByLabelText("名称")).not.toBeInTheDocument();
  });
  expect(screen.queryByText(tokenC)).not.toBeInTheDocument();
  expect(await screen.findByRole("button", { name: "复制 Desktop 的密钥" })).toBeInTheDocument();
  expect(
    fetchMock.mock.calls.some(
      ([input, init]) => init?.method === "POST" && String(input).includes("/rotate"),
    ),
  ).toBe(true);
  const rotateCall = fetchMock.mock.calls.find(
    ([input, init]) => init?.method === "POST" && String(input).includes("/rotate"),
  );
  expect(JSON.parse(String(rotateCall?.[1]?.body)).token).toBe(tokenC);
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
