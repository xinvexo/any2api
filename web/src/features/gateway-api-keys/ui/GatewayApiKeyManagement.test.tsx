import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { clearNotifications } from "@/shared/notifications";

import { GatewayApiKeyManagement } from "./GatewayApiKeyManagement";

const createdToken = `sk-${"a".repeat(43)}`;
const rotatedToken = `sk-${"b".repeat(43)}`;
const originalClipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");

afterEach(() => {
  clearNotifications();
  vi.restoreAllMocks();
  if (originalClipboard) {
    Object.defineProperty(navigator, "clipboard", originalClipboard);
  } else {
    Reflect.deleteProperty(navigator, "clipboard");
  }
});

test("keeps a pending create open and retains its one-time secret only in the view", async () => {
  let current = configuration(1, []);
  const createResponse = deferred<Response>();
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    if (String(input) === "/api/admin/gateway-api-keys" && init?.method === "POST") {
      return createResponse.promise;
    }
    return jsonResponse(current);
  });
  const { client } = renderManagement();

  expect(await screen.findByText("尚未创建网关密钥。客户端使用这些密钥访问本地网关。"))
    .toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "新增" }));
  fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "Created key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => {
    expect(screen.getByRole("button", { name: "关闭" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "关闭抽屉" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  });
  fireEvent.keyDown(window, { key: "Escape" });
  expect(screen.getByRole("dialog", { name: "新增" })).toBeInTheDocument();
  expect(postRequests(fetchMock)).toHaveLength(1);

  current = configuration(2, [wireKey(createdToken, 1, 1, "Created key")]);
  await act(async () => {
    createResponse.resolve(jsonResponse({ ...current, token: createdToken }));
    await createResponse.promise;
  });

  const secretDialog = await screen.findByRole("dialog", {
    name: "保存「Created key」的新密钥",
  });
  expect(secretDialog).toHaveTextContent(createdToken);
  expect(secretDialog.getElementsByTagName("code")).toHaveLength(1);
  const copy = screen.getByRole("button", { name: "复制密钥" });
  await waitFor(() => expect(copy).toHaveFocus());
  expect(JSON.stringify(client.getQueryCache().getAll().map((query) => query.state.data)))
    .not.toContain(createdToken);
  expect(client.getMutationCache().getAll().map((mutation) => mutation.state.data))
    .not.toContain(createdToken);

  fireEvent.click(screen.getByRole("button", { name: "已保存，关闭" }));
  await waitFor(() => expect(screen.queryByText(createdToken)).not.toBeInTheDocument());
  fireEvent.click(screen.getByRole("button", { name: "刷新" }));
  await waitFor(() => expect(fetchMock.mock.calls.length).toBeGreaterThan(2));
  expect(screen.queryByText(createdToken)).not.toBeInTheDocument();
});

test("remounts the rotate result so an immediate copy cannot rotate twice", async () => {
  let current = configuration(1, [wireKey(createdToken, 1, 1)]);
  const rotateResponse = deferred<Response>();
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    if (String(input).endsWith("/key-1/rotate") && init?.method === "POST") {
      return rotateResponse.promise;
    }
    return jsonResponse(current);
  });
  const { client } = renderManagement();

  await screen.findByText("Gateway key");
  fireEvent.click(screen.getByRole("button", { name: "轮换 Gateway key 的密钥" }));
  const confirm = await screen.findByRole("alertdialog", { name: "轮换「Gateway key」的密钥？" });
  fireEvent.click(screen.getByRole("button", { name: "确认轮换" }));

  await waitFor(() => {
    expect(screen.getByRole("button", { name: "取消" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "关闭对话框" })).toBeDisabled();
  });
  fireEvent.keyDown(window, { key: "Escape" });
  expect(confirm).toBeInTheDocument();
  expect(postRequests(fetchMock)).toHaveLength(1);

  current = configuration(2, [wireKey(rotatedToken, 2, 2)]);
  await act(async () => {
    rotateResponse.resolve(jsonResponse({ ...current, token: rotatedToken }));
    await rotateResponse.promise;
  });

  const secretDialog = await screen.findByRole("alertdialog", {
    name: "保存「Gateway key」的新密钥",
  });
  expect(secretDialog).toHaveTextContent(rotatedToken);
  expect(screen.getByRole("status")).toHaveTextContent("此密钥只显示一次");
  expect(JSON.stringify(client.getQueryCache().getAll().map((query) => query.state.data)))
    .not.toContain(rotatedToken);
  expect(client.getMutationCache().getAll().map((mutation) => mutation.state.data))
    .not.toContain(rotatedToken);

  fireEvent.click(screen.getByRole("button", { name: "复制密钥" }));
  await waitFor(() => expect(writeText).toHaveBeenCalledWith(rotatedToken));
  expect(postRequests(fetchMock)).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "已保存，关闭" }));
  await waitFor(() => expect(screen.queryByText(rotatedToken)).not.toBeInTheDocument());
});

test("keeps a failed create bound to its original revision after reconciliation", async () => {
  let current = configuration(1, []);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "POST") {
      current = configuration(2, []);
      return errorResponse(409, "configuration_revision_conflict", "configuration changed");
    }
    return jsonResponse(current);
  });
  const { client } = renderManagement();

  await screen.findByText("尚未创建网关密钥。客户端使用这些密钥访问本地网关。");
  fireEvent.click(screen.getByRole("button", { name: "新增" }));
  fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "Stale key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText(/当前草稿基于旧版本，不能直接保存/)).toBeInTheDocument();
  expect(screen.getByLabelText("名称")).toHaveValue("Stale key");
  expect(screen.getByLabelText("名称")).toBeEnabled();
  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "Revised draft" } });
  expect(screen.getByLabelText("名称")).toHaveValue("Revised draft");
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  expect(postRequests(fetchMock)).toHaveLength(1);
  expect(client.getMutationCache().getAll()).toHaveLength(0);
});

function renderManagement() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const router = createMemoryRouter(
    [{ path: "/keys", element: <GatewayApiKeyManagement /> }],
    { initialEntries: ["/keys"] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { client, router };
}

function configuration(configRevision: number, items: ReturnType<typeof wireKey>[]) {
  return { config_revision: configRevision, items };
}

function wireKey(
  token: string,
  tokenVersion: number,
  configVersion: number,
  name = "Gateway key",
) {
  return {
    id: "key-1",
    name,
    token_prefix: token.slice(0, 16),
    token_version: tokenVersion,
    config_version: configVersion,
    requests_per_minute: null,
    enabled: true,
    created_at: "2026-08-28 12:00:00",
    last_used_at: null,
    usage: {
      total_requests: 0,
      successful_requests: 0,
      failed_requests: 0,
      window_minutes: 2,
      window_slots: Array.from({ length: 30 }, (_, index) => ({
        started_at_ms: index * 120_000,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
      })),
    },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

function postRequests(fetchMock: { mock: { calls: Parameters<typeof fetch>[] } }) {
  return fetchMock.mock.calls.filter(([, init]) => init?.method === "POST");
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function errorResponse(status: number, code: string, message: string) {
  return new Response(JSON.stringify({ error: { code, message } }), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
