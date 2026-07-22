import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { SettingsManagement } from "./SettingsManagement";

afterEach(() => vi.restoreAllMocks());

test("shows merged sections and compact controls without override badges", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(configuration(1)));

  renderManagement();

  // Mock data only contains scheduler web groups → single merged section, no category chrome.
  expect(await screen.findByRole("heading", { name: "调度" })).toBeInTheDocument();
  expect(screen.queryByRole("navigation", { name: "设置分类" })).not.toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "排队策略" })).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "满载行为" })).toHaveValue("reject");

  const timeout = screen.getByRole("textbox", { name: "排队超时" });
  expect(timeout).toHaveValue("30");
  expect(timeout).toHaveAttribute("placeholder", "30");

  expect(screen.queryByText("已覆盖")).not.toBeInTheDocument();
  expect(screen.queryByText("未覆盖")).not.toBeInTheDocument();
  expect(screen.queryByText("默认")).not.toBeInTheDocument();
  expect(screen.queryByText("生效")).not.toBeInTheDocument();
  expect(screen.queryByText("覆盖")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "恢复排队超时默认值" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "恢复满载行为默认值" })).toBeInTheDocument();
});

test("saves and restores a setting using the visible revision", async () => {
  let current = configuration(1);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (init?.method === "PATCH") {
      current = configuration(2, 5);
    } else if (init?.method === "DELETE") {
      current = configuration(3);
    }
    return jsonResponse(path.includes("settings") ? current : {});
  });

  renderManagement();
  const input = await screen.findByRole("textbox", { name: "排队超时" });
  fireEvent.change(input, { target: { value: "5" } });
  fireEvent.click(screen.getByRole("button", { name: "保存排队超时" }));

  await waitFor(() => expect(screen.getByRole("textbox", { name: "排队超时" })).toHaveValue("5"));
  const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
  expect(JSON.parse(String(patch?.[1]?.body))).toEqual({
    expected_revision: 1,
    value: 5,
  });

  fireEvent.click(screen.getByRole("button", { name: "恢复排队超时默认值" }));
  await waitFor(() => expect(fetchMock.mock.calls.some(([, init]) => init?.method === "DELETE")).toBe(true));
  const remove = fetchMock.mock.calls.find(([, init]) => init?.method === "DELETE");
  expect(String(remove?.[0])).toContain("expected_revision=2");
  await waitFor(() => expect(screen.getByRole("textbox", { name: "排队超时" })).toHaveValue("30"));
});

test("keeps a draft after a revision conflict and retries with the refreshed revision", async () => {
  let getCount = 0;
  const revisions: number[] = [];
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      const body = JSON.parse(String(init.body)) as { expected_revision: number };
      revisions.push(body.expected_revision);
      if (revisions.length === 1) {
        return new Response(JSON.stringify({ error: { code: "revision_conflict", message: "configuration changed" } }), {
          status: 409,
          headers: { "Content-Type": "application/json" },
        });
      }
      return jsonResponse(configuration(3, 5));
    }
    getCount += 1;
    return jsonResponse(configuration(getCount === 1 ? 1 : 2));
  });

  renderManagement();
  const input = await screen.findByRole("textbox", { name: "排队超时" });
  fireEvent.change(input, { target: { value: "5" } });
  fireEvent.click(screen.getByRole("button", { name: "保存排队超时" }));

  expect(await screen.findByText("configuration changed")).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "排队超时" })).toHaveValue("5");
  fireEvent.click(screen.getByRole("button", { name: "保存排队超时" }));

  await waitFor(() => expect(revisions).toEqual([1, 2]));
  expect(fetchMock).toHaveBeenCalled();
});

test("can render only the affinity setting group", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(affinityConfiguration()));

  renderManagement("affinity.");

  expect(await screen.findByRole("heading", { name: "软会话粘性" })).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "软粘性模式" })).toHaveValue("prefer");
  expect(screen.getByRole("textbox", { name: "硬绑定 TTL" })).toHaveValue("86400");
  expect(screen.queryByRole("heading", { name: "排队策略" })).not.toBeInTheDocument();
  expect(screen.queryByRole("navigation", { name: "设置分类" })).not.toBeInTheDocument();
});

function renderManagement(keyPrefix?: string) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <SettingsManagement keyPrefix={keyPrefix} />
    </QueryClientProvider>,
  );
}

function affinityConfiguration() {
  return {
    config_revision: 1,
    items: [
      affinitySetting("affinity.soft.enabled", "boolean", true, null, null, "软会话粘性"),
      affinitySetting(
        "affinity.soft.mode",
        "enum",
        "prefer",
        null,
        ["prefer", "strict"],
        "软会话粘性",
      ),
      affinitySetting(
        "affinity.soft.ttl",
        "duration_secs",
        3600,
        null,
        null,
        "软会话粘性",
        1,
        604_800_000,
      ),
      affinitySetting(
        "affinity.hard.ttl",
        "duration_secs",
        86_400,
        null,
        null,
        "硬会话粘性",
        1,
        2592000,
      ),
    ],
  };
}

function affinitySetting(
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
    apply_mode: "hot_reload",
    web_group: webGroup,
    description: "Affinity test setting",
  };
}

function configuration(revision: number, timeoutOverride: number | null = null) {
  return {
    config_revision: revision,
    items: [
      setting("scheduler.on_saturated", "enum", "wait", "reject", ["wait", "reject"]),
      setting("scheduler.queue_timeout", "duration_secs", 30, timeoutOverride, null, 1, 86_400),
      setting("scheduler.max_waiting_requests", "integer", 128, null, null, 1, 100_000),
      setting("scheduler.fallback_on_saturation", "boolean", false, null, null),
      setting("scheduler.auxiliary_global_concurrency", "integer", 32, null, null, 1, 10_000),
      setting("scheduler.auxiliary_per_credential_concurrency", "integer", 4, null, null, 1, 10_000),
      setting("retry.max_total_attempts", "integer", 3, null, null, 1, 10),
      setting("retry.jitter_ratio", "integer", 20, null, null, 0, 100),
      setting("cooldown.rate_limit_fallback", "duration_secs", 60, null, null, 1, 86_400),
      setting("breaker.endpoint.failure_threshold", "integer", 3, null, null, 1, 100),
    ],
  };
}

function setting(
  key: string,
  valueType: string,
  defaultValue: boolean | number | string,
  overrideValue: boolean | number | string | null,
  allowedValues: string[] | null,
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
    apply_mode: "hot_reload",
    web_group: key.startsWith("scheduler.auxiliary") ? "辅助请求" : "排队策略",
    description: "Test setting",
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
