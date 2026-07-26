import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY } from "../model/system-log-auto-refresh-preference";
import { SystemLogManagement } from "./SystemLogManagement";

afterEach(() => {
  window.localStorage.removeItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY);
  vi.restoreAllMocks();
});

test("shows exact paths, automatic refresh choices, and clears with confirmation", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    if (String(input) === "/api/admin/system-logs" && init?.method === "DELETE") {
      return jsonResponse({ deleted: 1 });
    }
    if (String(input).startsWith("/api/admin/system-logs?limit=")) {
      return listResponse();
    }
    throw new Error(`unexpected request: ${String(input)}`);
  });

  renderManagement();

  expect(await screen.findByRole("table", { name: "系统日志表格" })).toBeInTheDocument();
  expect(screen.getByRole("list", { name: "系统日志列表" })).toBeInTheDocument();
  expect(screen.getAllByText("/api/admin/provider-credentials/actual-id").length).toBeGreaterThan(1);

  const autoRefresh = screen.getByRole("switch", { name: "自动刷新" });
  expect(autoRefresh).toHaveAttribute("aria-checked", "true");
  fireEvent.click(autoRefresh);
  expect(autoRefresh).toHaveAttribute("aria-checked", "false");

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
    if (String(input).startsWith("/api/admin/system-logs?limit=")) {
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

function renderManagement() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <SystemLogManagement />
    </QueryClientProvider>,
  );
}

function listResponse() {
  return jsonResponse({
    items: [
      {
        request_id: "11111111-1111-4111-8111-111111111111",
        started_at_ms: 1_700_000_000_000,
        config_revision: 3,
        client_ip: "203.0.113.8",
        method: "GET",
        path: "/api/admin/provider-credentials/actual-id",
        http_version: "HTTP/1.1",
        status_code: 200,
        duration_ms: 12,
        response_bytes: 42,
        outcome: "completed",
      },
    ],
    telemetry: { queued_records: 0, dropped_records: 0, persisted_records: 1 },
  });
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
