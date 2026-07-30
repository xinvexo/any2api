import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi, type MockInstance } from "vitest";

import { useSettingsEditor } from "../model/use-settings-editor";
import { SettingsManagement } from "./SettingsManagement";
import { SETTING_SECTIONS } from "./setting-categories";

afterEach(() => vi.restoreAllMocks());

test("shows frequent routing choices and folds low-frequency settings", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(configuration(1)));

  renderRoutingSettings();

  expect(await screen.findByRole("combobox", { name: "RPM 用尽行为" })).toHaveAttribute(
    "data-value",
    "reject",
  );
  expect(screen.getByRole("switch", { name: "启用会话粘性" })).toBeChecked();
  expect(screen.getByText("高级设置")).toBeInTheDocument();
  expect(screen.getByText("5 项")).toBeInTheDocument();
  const advanced = screen.getByText("高级设置").closest("details");
  expect(advanced).not.toHaveAttribute("open");

  fireEvent.click(screen.getByText("高级设置"));
  expect(advanced).toHaveAttribute("open");
  expect(screen.getByRole("textbox", { name: "排队超时" })).toHaveValue("30");
  expect(screen.getByRole("textbox", { name: "会话绑定 TTL" })).toHaveValue("86400");
  expect(screen.getByRole("textbox", { name: "会话绑定等待超时" })).toHaveValue("30");
  expect(screen.queryByRole("button", { name: "保存页面设置" })).not.toBeInTheDocument();
});

test("saves explicit overrides and exposes no restore-default action", async () => {
  let current = configuration(1);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      const body = JSON.parse(String(init.body)) as BatchBody;
      const timeout = body.updates.find(
        (update) => update.key === "scheduler.queue_timeout",
      )?.value;
      if (typeof timeout !== "number") throw new Error("missing queue timeout update");
      current = configuration(current.config_revision + 1, timeout);
    }
    return jsonResponse(current);
  });

  renderRoutingSettings();
  await screen.findByRole("combobox", { name: "RPM 用尽行为" });
  fireEvent.click(screen.getByText("高级设置"));
  const input = screen.getByRole("textbox", { name: "排队超时" });
  fireEvent.change(input, { target: { value: "5" } });
  fireEvent.click(screen.getByRole("button", { name: "保存页面设置" }));

  await waitFor(() => expect(screen.getByRole("textbox", { name: "排队超时" })).toHaveValue("5"));
  let patches = patchBodies(fetchMock);
  expect(patches[0]).toEqual({
    expected_revision: 1,
    updates: [{ key: "scheduler.queue_timeout", value: 5 }],
    resets: [],
  });
  expect(screen.queryByRole("button", { name: "保存页面设置" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /恢复.+默认值/ })).not.toBeInTheDocument();

  fireEvent.change(screen.getByRole("textbox", { name: "排队超时" }), {
    target: { value: "30" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存页面设置" }));

  await waitFor(() => {
    expect(screen.queryByRole("button", { name: "保存页面设置" })).not.toBeInTheDocument();
  });
  patches = patchBodies(fetchMock);
  expect(patches[1]).toEqual({
    expected_revision: 2,
    updates: [{ key: "scheduler.queue_timeout", value: 30 }],
    resets: [],
  });
  expect(screen.queryByRole("button", { name: /恢复.+默认值/ })).not.toBeInTheDocument();
});

test("keeps all drafts after a revision conflict and retries with refreshed revision", async () => {
  let getCount = 0;
  const revisions: number[] = [];
  vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      const body = JSON.parse(String(init.body)) as BatchBody;
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
  fireEvent.click(screen.getByRole("button", { name: "保存页面设置" }));

  expect(await screen.findByText(/configuration changed/)).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "排队超时" })).toHaveValue("5");
  fireEvent.click(screen.getByRole("button", { name: "保存页面设置" }));
  await waitFor(() => expect(revisions).toEqual([1, 2]));
});

test("searches, toggles, and batch-saves the global model allowlist", async () => {
  let current = modelConfiguration(1, null);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      const body = JSON.parse(String(init.body)) as BatchBody;
      const value = body.updates[0]?.value;
      current = modelConfiguration(current.config_revision + 1, Array.isArray(value) ? value : null);
    }
    return jsonResponse(current);
  });

  renderModelSettings();
  expect(await screen.findByText("全部 3 个模型可用")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("switch", { name: "允许全部公开模型" }));
  expect(screen.getByText("已允许 3 / 3")).toBeInTheDocument();
  const search = screen.getByRole("textbox", { name: "搜索可用模型" });
  fireEvent.change(search, { target: { value: "gpt" } });
  fireEvent.click(screen.getByRole("button", { name: "gpt-a" }));
  expect(screen.getByRole("button", { name: "gpt-a" })).toHaveAttribute("aria-pressed", "false");
  fireEvent.change(search, { target: { value: "" } });
  expect(screen.getByRole("group", { name: "可用模型" }).querySelector(".flex-wrap")).not.toBeNull();
  expect(screen.queryByRole("button", { name: "选择当前" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "清除当前" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "撤销客户端可使用模型修改" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "claude" }));
  fireEvent.click(screen.getByRole("button", { name: "保存页面设置" }));

  await waitFor(() => expect(screen.getByText("已允许 1 / 3")).toBeInTheDocument());
  let patches = patchBodies(fetchMock);
  expect(patches[0]).toEqual({
    expected_revision: 1,
    updates: [{ key: "models.allowed", value: ["gpt-b"] }],
    resets: [],
  });

  fireEvent.click(screen.getByRole("switch", { name: "允许全部公开模型" }));
  fireEvent.click(screen.getByRole("button", { name: "保存页面设置" }));
  await waitFor(() => expect(screen.getByText("全部 3 个模型可用")).toBeInTheDocument());
  patches = patchBodies(fetchMock);
  expect(patches[1]).toEqual({
    expected_revision: 2,
    updates: [{ key: "models.allowed", value: [] }],
    resets: [],
  });
});

test("shows friendly remote access copy and saves trusted proxy addresses", async () => {
  let current = basicConfiguration(1, null);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      const body = JSON.parse(String(init.body)) as BatchBody;
      const value = body.updates.find(
        (update) => update.key === "network.trusted_proxy_cidrs",
      )?.value;
      if (!Array.isArray(value)) throw new Error("missing trusted proxy update");
      current = basicConfiguration(current.config_revision + 1, value as string[]);
    }
    return jsonResponse(current);
  });

  renderBasicSettings();
  expect(await screen.findByRole("switch", { name: "允许远程管理" })).toBeChecked();
  expect(screen.getByText(/允许其他设备打开管理页面并登录/)).toBeInTheDocument();
  expect(screen.queryByText(/loopback|ANY2API_BIND/)).not.toBeInTheDocument();
  const proxies = screen.getByRole("textbox", { name: "可信反向代理地址" });
  expect(proxies).toHaveValue("");
  fireEvent.change(proxies, { target: { value: "127.0.0.1, 10.0.0.0/8" } });
  fireEvent.click(screen.getByRole("button", { name: "保存页面设置" }));

  await waitFor(() => expect(proxies).toHaveValue("10.0.0.0/8\n127.0.0.1"));
  expect(patchBodies(fetchMock)[0]).toEqual({
    expected_revision: 1,
    updates: [{
      key: "network.trusted_proxy_cidrs",
      value: ["10.0.0.0/8", "127.0.0.1"],
    }],
    resets: [],
  });
});

function renderRoutingSettings() {
  const section = SETTING_SECTIONS.find((item) => item.id === "routing");
  if (!section) throw new Error("missing routing setting section");
  return renderSettings(section.webGroups, section.featuredKeys);
}

function renderModelSettings() {
  return renderSettings(["公开模型"], ["models.allowed"]);
}

function renderBasicSettings() {
  const section = SETTING_SECTIONS.find((item) => item.id === "basic");
  if (!section) throw new Error("missing basic setting section");
  return renderSettings(section.webGroups, section.featuredKeys);
}

function renderSettings(webGroups: readonly string[], featuredKeys: readonly string[]) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <SettingsHarness webGroups={webGroups} featuredKeys={featuredKeys} />
    </QueryClientProvider>,
  );
}

function SettingsHarness({
  webGroups,
  featuredKeys,
}: {
  webGroups: readonly string[];
  featuredKeys: readonly string[];
}) {
  const editor = useSettingsEditor(webGroups);
  return (
    <>
      {editor.isDirty ? (
        <button
          type="button"
          disabled={editor.pending || editor.hasValidationErrors}
          onClick={() => void editor.save()}
        >
          保存页面设置
        </button>
      ) : null}
      <SettingsManagement
        editor={editor}
        featuredKeys={featuredKeys}
        showSectionHeading={false}
      />
    </>
  );
}

interface BatchBody {
  expected_revision: number;
  updates: Array<{ key: string; value: unknown }>;
  resets: string[];
}

function patchBodies(mock: MockInstance<typeof fetch>) {
  return mock.mock.calls
    .filter(([, init]) => init?.method === "PATCH")
    .map(([, init]) => JSON.parse(String(init?.body)) as BatchBody);
}

function configuration(revision: number, timeoutOverride: number | null = null) {
  return {
    config_revision: revision,
    items: [
      setting("scheduler.on_rate_limited", "enum", "wait", "reject", ["wait", "reject"], "排队策略"),
      setting("scheduler.queue_timeout", "duration_secs", 30, timeoutOverride, null, "排队策略", 1, 86_400),
      setting("scheduler.max_waiting_requests", "integer", 128, null, null, "排队策略", 1, 100_000),
      setting("scheduler.fallback_on_rate_limit", "boolean", false, null, null, "排队策略"),
      setting("affinity.enabled", "boolean", true, null, null, "会话粘性"),
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
      "string_list",
      [],
      override,
      null,
      "公开模型",
      null,
      null,
      ["claude", "gpt-a", "gpt-b"],
    )],
  };
}

function basicConfiguration(revision: number, trustedProxies: string[] | null) {
  return {
    config_revision: revision,
    items: [
      setting(
        "admin.remote_enabled",
        "boolean",
        true,
        null,
        null,
        "远程管理",
        null,
        null,
        null,
        "允许其他设备打开管理页面并登录。关闭后，只有运行 any2api 的这台设备可以访问管理功能。",
      ),
      setting(
        "network.trusted_proxy_cidrs",
        "string_list",
        [],
        trustedProxies,
        null,
        "远程管理",
        null,
        null,
        null,
        "使用反向代理时填写代理服务器地址；没有反向代理请留空。",
      ),
    ],
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
  description = "Test setting",
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
    description,
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
