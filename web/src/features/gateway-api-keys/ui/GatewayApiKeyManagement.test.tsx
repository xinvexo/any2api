import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { GatewayApiKeyManagement } from "./GatewayApiKeyManagement";

afterEach(() => vi.restoreAllMocks());

const tokenA = `a2k_v1_${"b".repeat(43)}`;
const tokenB = `a2k_v1_${"c".repeat(43)}`;
const tokenC = `a2k_v1_${"d".repeat(43)}`;

test("creates a gateway key with a server-generated token and exposes copy in row actions", async () => {
  let configuration: Record<string, unknown> = { config_revision: 1, items: [] };
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "POST") {
      const body = JSON.parse(String(init.body)) as { name: string };
      configuration = {
        config_revision: 2,
        items: [
          {
            id: "key-1",
            name: body.name,
            token: tokenA,
            token_prefix: tokenA.slice(0, 16),
            token_version: 1,
            config_version: 1,
            enabled: true,
            created_at: "2026-07-19 10:00:00",
            last_used_at: null,
            usage: usage(),
          },
        ],
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
  expect(screen.queryByLabelText("密钥")).not.toBeInTheDocument();
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
    enabled: true,
  });
  expect(JSON.parse(String(createCall?.[1]?.body))).not.toHaveProperty("token");
  expect(screen.getByTestId("location")).not.toHaveTextContent(tokenA);
});

test("lists keys with a real time window and hover or focus details", async () => {
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
          created_at: "2026-07-19 10:00:00",
          last_used_at: null,
          usage: usage({
            total_requests: 177,
            successful_requests: 134,
            failed_requests: 43,
            window_slots: usageSlots({
              28: { total_requests: 2, successful_requests: 2, failed_requests: 0 },
              29: { total_requests: 2, successful_requests: 1, failed_requests: 1 },
            }),
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

  const caption = await screen.findByRole("caption", { name: "网关密钥列表" });
  const table = caption.closest("table");
  expect(table).toHaveAttribute("data-responsive-table", "cards");
  expect(within(table!).getAllByRole("row")).toHaveLength(2);
  const keyRow = within(table!).getByText("Desktop").closest("tr");
  expect(keyRow).toHaveAttribute("data-responsive-row", "card");
  expect(within(keyRow!).getByText("调用统计")).toBeInTheDocument();
  expect(within(keyRow!).getByText("最后使用")).toBeInTheDocument();
  expect(within(keyRow!).getByText("创建时间")).toBeInTheDocument();
  expect(screen.getByText("Desktop")).toBeInTheDocument();
  expect(screen.queryByText(tokenA)).not.toBeInTheDocument();
  expect(screen.queryByRole("columnheader", { name: "密钥" })).not.toBeInTheDocument();
  expect(screen.queryByRole("columnheader", { name: "状态" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "复制 Desktop 的密钥" })).toBeInTheDocument();
  expect(screen.getByRole("switch", { name: "禁用 Desktop" })).toHaveAttribute(
    "aria-checked",
    "true",
  );
  expect(screen.getByRole("button", { name: "轮换 Desktop 的密钥" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "删除 Desktop" })).toBeInTheDocument();
  expect(screen.getByText("成功 134")).toBeInTheDocument();
  expect(screen.getByText("失败 43")).toBeInTheDocument();
  expect(screen.queryByText("暂无调用")).not.toBeInTheDocument();

  const timeline = screen.getByRole("group", { name: /Desktop 近 1 小时，每格 2 分钟/ });
  const slots = within(timeline).getAllByRole("button");
  expect(slots).toHaveLength(30);
  expect(slots[29]).toHaveAccessibleName(/成功 1，失败 1/);

  fireEvent.mouseEnter(slots[29]);
  let tooltip = await screen.findByRole("tooltip");
  expect(within(tooltip).getByText("成功 1")).toBeInTheDocument();
  expect(within(tooltip).getByText("失败 1")).toBeInTheDocument();
  fireEvent.mouseLeave(timeline);
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();

  fireEvent.focus(slots[28]);
  tooltip = await screen.findByRole("tooltip");
  expect(within(tooltip).getByText("成功 2")).toBeInTheDocument();
  expect(within(tooltip).getByText("失败 0")).toBeInTheDocument();
});

test("toggles enabled from the row switch without opening the editor", async () => {
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

  const enabledSwitch = await screen.findByRole("switch", { name: "禁用 Desktop" });
  expect(enabledSwitch).toHaveAttribute("aria-checked", "true");
  fireEvent.click(enabledSwitch);

  await waitFor(() => {
    expect(screen.getByRole("switch", { name: "启用 Desktop" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });
  const patchCall = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
  expect(JSON.parse(String(patchCall?.[1]?.body))).toMatchObject({
    name: "Desktop",
    enabled: false,
  });
});

test("rotates from an explicit confirmation and never sends client token material", async () => {
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
            token: tokenC,
            token_prefix: tokenC.slice(0, 16),
            token_version: 2,
            config_version: 2,
            enabled: true,
            created_at: "2026-07-19 10:00:00",
            last_used_at: null,
            usage: usage(),
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

  fireEvent.click(await screen.findByRole("button", { name: "轮换 Desktop 的密钥" }));
  expect(
    await screen.findByText("服务端会生成新的强随机 token，旧 token 在发布完成后立即失效。"),
  ).toBeInTheDocument();
  fireEvent.click(await screen.findByRole("button", { name: "确认轮换" }));

  await waitFor(() => expect(screen.queryByRole("button", { name: "确认轮换" })).not.toBeInTheDocument());
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
  expect(JSON.parse(String(rotateCall?.[1]?.body))).toEqual({
    expected_revision: 3,
    expected_config_version: 1,
    expected_token_version: 1,
  });
});

test("deletes a key with DELETE and CAS query parameters", async () => {
  let configuration: Record<string, unknown> = {
    config_revision: 4,
    items: [
      {
        id: "key-1",
        name: "Desktop",
        token: tokenA,
        token_prefix: tokenA.slice(0, 16),
        token_version: 2,
        config_version: 3,
        enabled: true,
        created_at: "2026-07-19 10:00:00",
        last_used_at: null,
        usage: usage(),
      },
    ],
  };
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "DELETE") {
      configuration = { config_revision: 5, items: [] };
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

  fireEvent.click(await screen.findByRole("button", { name: "删除 Desktop" }));
  fireEvent.click(await screen.findByRole("button", { name: "确认删除" }));
  await waitFor(() => expect(screen.queryByText("Desktop")).not.toBeInTheDocument());

  const deleteCall = fetchMock.mock.calls.find(([, init]) => init?.method === "DELETE");
  expect(String(deleteCall?.[0])).toContain(
    "/api/admin/gateway-api-keys/key-1?expected_revision=4&expected_config_version=3",
  );
  expect(deleteCall?.[1]?.body).toBeUndefined();
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
    window_minutes: 2,
    window_slots: usageSlots(),
    ...overrides,
  };
}

function usageSlots(
  overrides: Record<
    number,
    { total_requests: number; successful_requests: number; failed_requests: number }
  > = {},
) {
  return Array.from({ length: 30 }, (_, index) => {
    const slot = overrides[index];
    return {
      started_at_ms: 1_720_000_000_000 + index * 120_000,
      total_requests: slot?.total_requests ?? 0,
      successful_requests: slot?.successful_requests ?? 0,
      failed_requests: slot?.failed_requests ?? 0,
    };
  });
}
