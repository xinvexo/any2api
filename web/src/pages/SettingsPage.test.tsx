import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { SettingsPage } from "./SettingsPage";
import { clearNotifications, getNotifications } from "@/shared/notifications";

afterEach(() => {
  clearNotifications();
  vi.restoreAllMocks();
});

test("puts refresh and conditional batch save in the fixed page toolbar", async () => {
  let current = configuration(1, true);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      current = configuration(2, false);
    }
    return jsonResponse(current);
  });
  renderSettingsPage();

  const affinity = await screen.findByRole("switch", { name: "启用会话粘性" });
  expect(screen.getByRole("region", { name: "路由策略" }).closest(".management-scroll-viewport"))
    .not.toBeNull();
  expect(screen.getAllByRole("button", { name: "刷新当前设置页" })).toHaveLength(1);
  expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
  expect(getNotifications()).toHaveLength(0);

  fireEvent.click(screen.getByRole("button", { name: "刷新当前设置页" }));
  await waitFor(() => {
    expect(getNotifications().map((item) => item.message)).toEqual(["设置已刷新"]);
  });
  clearNotifications();

  fireEvent.click(affinity);
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument());
  expect(getNotifications()).toEqual([
    expect.objectContaining({ message: "设置已保存", tone: "success" }),
  ]);
  const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
  expect(JSON.parse(String(patch?.[1]?.body))).toEqual({
    expected_revision: 1,
    updates: [{ key: "affinity.enabled", value: false }],
    resets: [],
  });
});

test("offers save, discard, and cancel before refresh or navigation", async () => {
  let current = configuration(1, true);
  let getCount = 0;
  vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      current = configuration(2, false);
      return jsonResponse(current);
    }
    getCount += 1;
    return jsonResponse(current);
  });
  renderSettingsPage();

  fireEvent.click(await screen.findByRole("switch", { name: "启用会话粘性" }));
  const unload = new Event("beforeunload", { cancelable: true });
  window.dispatchEvent(unload);
  expect(unload.defaultPrevented).toBe(true);

  fireEvent.click(screen.getByRole("link", { name: "运行保护" }));
  expect(await screen.findByRole("alertdialog", { name: "离开前保存修改？" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "保存并离开" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "放弃修改" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(screen.getByRole("link", { name: "路由策略" })).toHaveAttribute("aria-current", "page");

  fireEvent.click(screen.getByRole("button", { name: "刷新当前设置页" }));
  expect(await screen.findByRole("alertdialog", { name: "刷新前保存修改？" })).toBeInTheDocument();
  const requestsBeforeRefresh = getCount;
  fireEvent.click(screen.getByRole("button", { name: "放弃并刷新" }));
  await waitFor(() => expect(getCount).toBeGreaterThan(requestsBeforeRefresh));
  expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
  expect(getNotifications().map((item) => item.message)).toEqual(["设置已刷新"]);

  fireEvent.click(screen.getByRole("switch", { name: "启用会话粘性" }));
  fireEvent.click(screen.getByRole("link", { name: "运行保护" }));
  fireEvent.click(await screen.findByRole("button", { name: "保存并离开" }));
  await waitFor(() => {
    expect(screen.getByRole("link", { name: "运行保护" })).toHaveAttribute("aria-current", "page");
  });
  expect(getNotifications().map((item) => item.message)).toEqual([
    "设置已保存",
    "设置已刷新",
  ]);
});

function renderSettingsPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const router = createMemoryRouter([
    { path: "/settings/:section", element: <SettingsPage /> },
  ], { initialEntries: ["/settings/routing"] });
  return render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
}

function configuration(revision: number, affinityEnabled: boolean) {
  return {
    config_revision: revision,
    items: [
      setting("scheduler.on_rate_limited", "enum", "wait", "reject", ["wait", "reject"], "排队策略"),
      setting("scheduler.queue_timeout", "duration_secs", 30, null, null, "排队策略", 1, 86_400),
      setting("scheduler.max_waiting_requests", "integer", 128, null, null, "排队策略", 1, 100_000),
      setting("scheduler.fallback_on_rate_limit", "boolean", false, null, null, "排队策略"),
      setting("affinity.enabled", "boolean", true, affinityEnabled, null, "会话粘性"),
      setting("affinity.ttl", "duration_secs", 86_400, null, null, "会话粘性", 1, 2_592_000),
      setting("affinity.wait_timeout", "duration_secs", 30, null, null, "会话粘性", 1, 86_400),
    ],
  };
}

function setting(
  key: string,
  valueType: string,
  defaultValue: boolean | number | string,
  overrideValue: boolean | number | string | null,
  allowedValues: string[] | null,
  webGroup: string,
  minValue: number | null = null,
  maxValue: number | null = null,
) {
  return {
    key,
    value_type: valueType,
    default_value: defaultValue,
    override_value: overrideValue,
    effective_value: overrideValue ?? defaultValue,
    min_value: minValue,
    max_value: maxValue,
    allowed_values: allowedValues,
    options: null,
    apply_mode: "hot_reload",
    web_group: webGroup,
    description: "Test setting",
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
