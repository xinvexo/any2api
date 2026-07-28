import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { SettingsManagement } from "./SettingsManagement";
import { SETTING_SECTIONS } from "./setting-categories";

afterEach(() => vi.restoreAllMocks());

test("shows frequent routing choices and folds low-frequency settings", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(configuration(1)));

  renderRoutingSettings();

  expect(await screen.findByRole("combobox", { name: "RPM 用尽行为" })).toHaveValue("reject");
  expect(screen.getByText("高级设置")).toBeInTheDocument();
  expect(screen.getByText("5 项")).toBeInTheDocument();
  const advanced = screen.getByText("高级设置").closest("details");
  expect(advanced).not.toHaveAttribute("open");

  fireEvent.click(screen.getByText("高级设置"));
  expect(advanced).toHaveAttribute("open");
  expect(screen.getByRole("textbox", { name: "排队超时" })).toHaveValue("30");
  expect(screen.getByRole("textbox", { name: "会话绑定 TTL" })).toHaveValue("86400");
  expect(screen.getByRole("textbox", { name: "会话绑定等待超时" })).toHaveValue("30");
  expect(screen.queryByText(/软粘性|硬粘性|Prefer/)).not.toBeInTheDocument();
  expect(screen.queryByText("已覆盖")).not.toBeInTheDocument();
  expect(screen.queryByText("未覆盖")).not.toBeInTheDocument();
});

test("saves and restores an advanced setting using the visible revision", async () => {
  let current = configuration(1);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      current = configuration(2, 5);
    } else if (init?.method === "DELETE") {
      current = configuration(3);
    }
    return jsonResponse(current);
  });

  renderRoutingSettings();
  await screen.findByRole("combobox", { name: "RPM 用尽行为" });
  fireEvent.click(screen.getByText("高级设置"));
  const input = screen.getByRole("textbox", { name: "排队超时" });
  fireEvent.change(input, { target: { value: "5" } });
  fireEvent.click(screen.getByRole("button", { name: "保存排队超时" }));

  await waitFor(() => expect(screen.getByRole("textbox", { name: "排队超时" })).toHaveValue("5"));
  const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
  expect(JSON.parse(String(patch?.[1]?.body))).toEqual({ expected_revision: 1, value: 5 });

  fireEvent.click(screen.getByRole("button", { name: "恢复排队超时默认值" }));
  await waitFor(() => expect(fetchMock.mock.calls.some(([, init]) => init?.method === "DELETE")).toBe(true));
  const remove = fetchMock.mock.calls.find(([, init]) => init?.method === "DELETE");
  expect(String(remove?.[0])).toContain("expected_revision=2");
  await waitFor(() => expect(screen.getByRole("textbox", { name: "排队超时" })).toHaveValue("30"));
});

test("keeps an advanced draft after a revision conflict", async () => {
  let getCount = 0;
  const revisions: number[] = [];
  vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      const body = JSON.parse(String(init.body)) as { expected_revision: number };
      revisions.push(body.expected_revision);
      if (revisions.length === 1) {
        return new Response(
          JSON.stringify({ error: { code: "revision_conflict", message: "configuration changed" } }),
          { status: 409, headers: { "Content-Type": "application/json" } },
        );
      }
      return jsonResponse(configuration(3, 5));
    }
    getCount += 1;
    return jsonResponse(configuration(getCount === 1 ? 1 : 2));
  });

  renderRoutingSettings();
  await screen.findByRole("combobox", { name: "RPM 用尽行为" });
  fireEvent.click(screen.getByText("高级设置"));
  const input = screen.getByRole("textbox", { name: "排队超时" });
  fireEvent.change(input, { target: { value: "5" } });
  fireEvent.click(screen.getByRole("button", { name: "保存排队超时" }));

  expect(await screen.findByText("configuration changed")).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "排队超时" })).toHaveValue("5");
  fireEvent.click(screen.getByRole("button", { name: "保存排队超时" }));
  await waitFor(() => expect(revisions).toEqual([1, 2]));
});

test("searches, selects, clears, and saves the global model allowlist", async () => {
  let current = modelConfiguration(1, null);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      current = modelConfiguration(2, ["gpt-b"]);
    }
    return jsonResponse(current);
  });

  renderModelSettings();
  expect(await screen.findByText("全部 3 个模型可用")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("switch", { name: "允许全部公开模型" }));
  expect(screen.getByText("已拒绝全部模型")).toBeInTheDocument();
  const search = screen.getByRole("textbox", { name: "搜索可用模型" });
  fireEvent.change(search, { target: { value: "gpt" } });
  fireEvent.click(screen.getByRole("button", { name: "选择当前" }));
  expect(screen.getByRole("checkbox", { name: "gpt-a" })).toBeChecked();
  expect(screen.getByRole("checkbox", { name: "gpt-b" })).toBeChecked();
  fireEvent.click(screen.getByRole("button", { name: "清除当前" }));
  expect(screen.getByRole("checkbox", { name: "gpt-a" })).not.toBeChecked();
  fireEvent.click(screen.getByRole("checkbox", { name: "gpt-b" }));
  fireEvent.click(screen.getByRole("button", { name: "保存可使用的模型" }));

  await waitFor(() => expect(screen.getByText("已允许 1 / 3")).toBeInTheDocument());
  const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
  expect(JSON.parse(String(patch?.[1]?.body))).toEqual({
    expected_revision: 1,
    value: ["gpt-b"],
  });
});

function renderRoutingSettings() {
  const section = SETTING_SECTIONS.find((item) => item.id === "routing");
  if (!section) throw new Error("missing routing setting section");
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <SettingsManagement
        webGroups={section.webGroups}
        featuredKeys={section.featuredKeys}
        showSectionHeading={false}
      />
    </QueryClientProvider>,
  );
}

function renderModelSettings() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <SettingsManagement
        webGroups={["公开模型"]}
        featuredKeys={["models.allowed"]}
        showSectionHeading={false}
      />
    </QueryClientProvider>,
  );
}

function configuration(revision: number, timeoutOverride: number | null = null) {
  return {
    config_revision: revision,
    items: [
      setting("scheduler.on_rate_limited", "enum", "wait", "reject", ["wait", "reject"], "排队策略"),
      setting("scheduler.queue_timeout", "duration_secs", 30, timeoutOverride, null, "排队策略", 1, 86_400),
      setting("scheduler.max_waiting_requests", "integer", 128, null, null, "排队策略", 1, 100_000),
      setting("scheduler.fallback_on_rate_limit", "boolean", false, null, null, "排队策略"),
      setting("affinity.ttl", "duration_secs", 86_400, null, null, "会话粘性", 1, 2_592_000),
      setting("affinity.wait_timeout", "duration_secs", 30, null, null, "会话粘性", 1, 86_400),
    ],
  };
}

function modelConfiguration(revision: number, override: string[] | null) {
  return {
    config_revision: revision,
    items: [setting(
      "models.allowed",
      "optional_string_list",
      null,
      override,
      null,
      "公开模型",
      null,
      null,
      ["claude", "gpt-a", "gpt-b"],
    )],
  };
}

function setting(
  key: string,
  valueType: string,
  defaultValue: boolean | number | string | string[] | null,
  overrideValue: boolean | number | string | string[] | null,
  allowedValues: string[] | null,
  webGroup: string,
  minValue: number | null = null,
  maxValue: number | null = null,
  options: string[] | null = null,
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
    options,
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
