import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY } from "../model/system-log-auto-refresh-preference";
import { SystemLogManagement } from "./SystemLogManagement";
import { FakeEventSource } from "@/test/fake-event-source";

afterEach(() => {
  window.localStorage.removeItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY);
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  FakeEventSource.reset();
});

test("shows exact paths, automatic refresh choices, and clears with confirmation", async () => {
  vi.stubGlobal("EventSource", FakeEventSource);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    if (String(input) === "/api/admin/system-logs" && init?.method === "DELETE") {
      return jsonResponse({ deleted: 1 });
    }
    if (String(input).startsWith("/api/admin/system-logs?page=")) {
      return listResponse();
    }
    throw new Error(`unexpected request: ${String(input)}`);
  });

  renderManagement();

  expect(await screen.findByRole("table", { name: "系统日志表格" })).toBeInTheDocument();
  expect(screen.getByRole("list", { name: "系统日志列表" })).toBeInTheDocument();
  expect(screen.getAllByText("/api/admin/provider-credentials/actual-id").length).toBeGreaterThan(1);
  expect(screen.queryByText("配置版本")).not.toBeInTheDocument();
  expect(screen.queryByTitle(/Config revision/i)).not.toBeInTheDocument();
  expect(
    screen.getByTitle("Request ID: 11111111-1111-4111-8111-111111111111"),
  ).toBeInTheDocument();

  await act(async () => {
    FakeEventSource.instances[0]?.emit("system_logs_changed");
  });
  await waitFor(() => {
    expect(
      fetchMock.mock.calls.filter(([input]) =>
        String(input).startsWith("/api/admin/system-logs?page="),
      ),
    ).toHaveLength(2);
  });

  const autoRefresh = screen.getByRole("switch", { name: "自动刷新" });
  expect(autoRefresh).toHaveAttribute("aria-checked", "true");
  expect(FakeEventSource.instances).toHaveLength(1);
  expect(FakeEventSource.instances[0]?.url).toBe("/api/admin/log-events");
  fireEvent.click(autoRefresh);
  expect(autoRefresh).toHaveAttribute("aria-checked", "false");
  expect(FakeEventSource.instances[0]?.closed).toBe(true);

  fireEvent.click(screen.getByRole("button", { name: "清理历史日志" }));
  const dialog = await screen.findByRole("alertdialog");
  fireEvent.click(dialog.querySelector<HTMLButtonElement>('button[data-variant="dangerSolid"]')!);

  await waitFor(() => {
    expect(
      fetchMock.mock.calls.some(
        ([input, init]) => String(input) === "/api/admin/system-logs" && init?.method === "DELETE",
      ),
    ).toBe(true);
  });
});

test("persists the automatic refresh choice across remounts", async () => {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    if (String(input).startsWith("/api/admin/system-logs?page=")) {
      return listResponse();
    }
    throw new Error(`unexpected request: ${String(input)}`);
  });

  const firstRender = renderManagement();
  const firstSwitch = await screen.findByRole("switch", { name: "自动刷新" });
  expect(firstSwitch).toHaveAttribute("aria-checked", "true");

  fireEvent.click(firstSwitch);
  expect(firstSwitch).toHaveAttribute("aria-checked", "false");
  expect(window.localStorage.getItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY)).toBe("false");

  firstRender.unmount();
  renderManagement();

  const restoredSwitch = await screen.findByRole("switch", { name: "自动刷新" });
  expect(restoredSwitch).toHaveAttribute("aria-checked", "false");

  fireEvent.click(restoredSwitch);
  expect(restoredSwitch).toHaveAttribute("aria-checked", "true");
  expect(window.localStorage.getItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY)).toBe("true");
});

test("defaults automatic refresh to enabled for an invalid saved value", async () => {
  window.localStorage.setItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY, "invalid");
  vi.spyOn(globalThis, "fetch").mockResolvedValue(listResponse());

  renderManagement();

  expect(await screen.findByRole("switch", { name: "自动刷新" })).toHaveAttribute(
    "aria-checked",
    "true",
  );
});

test("paginates system logs on the server", async () => {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const path = String(input);
    if (path === "/api/admin/system-logs?page=1&page_size=20") {
      return listResponse("/v1/responses", 21, 1, 20);
    }
    if (path === "/api/admin/system-logs?page=2&page_size=20") {
      return listResponse("/v1/models", 21, 2, 20);
    }
    throw new Error(`unexpected request: ${path}`);
  });

  renderManagement();
  expect((await screen.findAllByText("/v1/responses")).length).toBeGreaterThan(1);
  expect(screen.getByText("共 21 条")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  expect((await screen.findAllByText("/v1/models")).length).toBeGreaterThan(1);
  expect(screen.queryByText("/v1/responses")).not.toBeInTheDocument();
});

function renderManagement() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <SystemLogManagement />
    </QueryClientProvider>,
  );
}

function listResponse(
  path = "/api/admin/provider-credentials/actual-id",
  total = 1,
  page = 1,
  pageSize = 20,
) {
  return jsonResponse({
    items: [
      {
        request_id: "11111111-1111-4111-8111-111111111111",
        started_at_ms: 1_700_000_000_000,
        config_revision: 3,
        client_ip: "203.0.113.8",
        method: "GET",
        path,
        http_version: "HTTP/1.1",
        status_code: 200,
        duration_ms: 12,
        response_bytes: 42,
        outcome: "completed",
      },
    ],
    total,
    page,
    page_size: pageSize,
    telemetry: { queued_records: 0, dropped_records: 0, persisted_records: 1 },
  });
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
