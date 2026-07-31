import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import {
  configuration,
  credential,
  credentialConfiguration,
  endpoint,
  jsonResponse,
  mockAdminApis,
  renderManagement,
} from "./ProviderManagement.test-support";

afterEach(() => vi.restoreAllMocks());

test("shows the empty Provider state", async () => {
  mockAdminApis(() => configuration(1, []));

  renderManagement();

  expect(await screen.findByText("还没有 Codex Endpoint")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "新增" })).toBeInTheDocument();
  expect(screen.queryByText(/配置版本/)).not.toBeInTheDocument();
});

test("shows a stable API Key skeleton while an endpoint accordion loads", async () => {
  let resolveCredentials!: (response: Response) => void;
  const pendingCredentials = new Promise<Response>((resolve) => {
    resolveCredentials = resolve;
  });
  mockAdminApis(
    () => configuration(1, [endpoint()]),
    () => credentialConfiguration(1, [credential()]),
    (input) => String(input).includes("/credentials") ? pendingCredentials : null,
  );

  renderManagement();

  fireEvent.click(
    await screen.findByRole("button", { name: "展开 Codex Primary 的 API Key" }),
  );
  const loading = await screen.findByRole("status", { name: "正在读取 API Key 配置" });
  expect(loading.querySelectorAll("[data-skeleton]").length).toBeGreaterThan(0);
  expect(loading.parentElement).toHaveClass("col-span-2", "sm:col-span-1");
  expect(loading.parentElement?.previousElementSibling).toHaveClass("hidden", "sm:block");
  expect(loading.parentElement?.parentElement).not.toHaveClass("px-2.5");
  expect(loading.parentElement?.parentElement).toHaveClass("sm:px-3");
  expect(loading.children[1]).toHaveClass("pl-[2.125rem]", "pr-2.5", "sm:px-0");

  await act(async () => {
    resolveCredentials(jsonResponse(credentialConfiguration(1, [credential()])));
  });
  expect(screen.getByRole("status", { name: "正在读取 API Key 配置" })).toBeInTheDocument();
  expect(screen.queryByText("Primary Key")).not.toBeInTheDocument();
  const credentialName = await screen.findByText("Primary Key");
  expect(credentialName).toBeInTheDocument();
  const credentialCard = credentialName.closest("[data-floating-bounds]");
  expect(credentialCard).toHaveClass("pl-[2.125rem]", "pr-2.5", "sm:px-0");
  expect(credentialCard?.closest(".col-span-2")).toHaveClass(
    "col-span-2",
    "sm:col-span-1",
  );
  expect(screen.queryByRole("status", { name: "正在读取 API Key 配置" })).not.toBeInTheDocument();
});

test("expands endpoint accordion to show nested API keys on the same page", async () => {
  const fetchMock = mockAdminApis(
    () => configuration(1, [endpoint()]),
    () => credentialConfiguration(3, [credential()]),
  );

  renderManagement();

  const header = await screen.findByRole("button", { name: "展开 Codex Primary 的 API Key" });
  expect(header).toHaveAttribute("aria-expanded", "false");
  expect(screen.queryByText("Primary Key")).not.toBeInTheDocument();
  expect(screen.queryByRole("link", { name: /API Key/ })).not.toBeInTheDocument();

  fireEvent.click(header);

  expect(await screen.findByRole("button", { name: "收起 Codex Primary 的 API Key" })).toHaveAttribute(
    "aria-expanded",
    "true",
  );
  expect(screen.getByRole("region", { name: "Codex Primary 的 API Key" })).toHaveClass(
    "bg-surface/45",
  );
  expect(await screen.findByText("Primary Key")).toBeInTheDocument();
  expect(screen.getByText("成功 2")).toBeInTheDocument();
  expect(screen.getByText("失败 1")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "配置 Primary Key 的模型" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "编辑 Primary Key" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "删除 Primary Key" })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "测试 Primary Key" })).not.toBeInTheDocument();
  expect(fetchMock.mock.calls.map(([input]) => String(input))).toEqual(
    expect.arrayContaining([
      "/api/admin/provider-endpoints",
      expect.stringContaining("/credentials"),
      "/api/admin/proxies",
    ]),
  );
});

test("uses icon-only endpoint actions and toggles an endpoint inline", async () => {
  let current = configuration(1, [endpoint()]);
  const fetchMock = mockAdminApis(
    () => current,
    () => credentialConfiguration(1, []),
    (input, init) => {
      if (String(input).includes("/provider-endpoints/") && init?.method === "PATCH") {
        current = configuration(2, [endpoint({ enabled: false, config_version: 2 })]);
        return jsonResponse(current);
      }
      return null;
    },
  );

  renderManagement();

  const create = await screen.findByRole("button", {
    name: "新增 Codex Primary 的 API Key",
  });
  const expand = screen.getByRole("button", { name: "展开 Codex Primary 的 API Key" });
  const edit = screen.getByRole("button", { name: "编辑 Codex Primary" });
  const disable = screen.getByRole("button", { name: "停用 Codex Primary" });

  expect(expand.parentElement).toHaveClass("sm:items-center");
  expect(expand).not.toHaveClass("hover:bg-surface-muted/50");
  expect(expand).not.toHaveClass("active:bg-surface-muted/70");
  expect(create).not.toHaveTextContent("新增");
  expect(edit).not.toHaveTextContent("编辑");
  expect(create).toHaveAttribute("title", "新增 Codex Primary 的 API Key");
  expect(edit).toHaveAttribute("title", "编辑 Codex Primary");

  fireEvent.click(disable);

  expect(await screen.findByText("已停用")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "启用 Codex Primary" })).toBeInTheDocument();
  await waitFor(() => {
    const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
    expect(JSON.parse(String(patch?.[1]?.body))).toEqual({
      expected_revision: 1,
      expected_config_version: 1,
      name: "Codex Primary",
      provider_kind: "codex",
      base_url: "https://api.example.com/v1",
      protocol_dialect: "openai_responses",
      upstream_protocol_dialect: null,
      enabled: false,
    });
  });
});
