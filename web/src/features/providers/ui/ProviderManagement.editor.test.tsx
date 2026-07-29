import { fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import {
  configuration,
  credentialConfiguration,
  endpoint,
  jsonResponse,
  mockAdminApis,
  renderManagement,
} from "./ProviderManagement.test-support";

afterEach(() => vi.restoreAllMocks());

test("creates a Claude private HTTP endpoint directly from the Base URL", async () => {
  let current = configuration(1, []);
  const fetchMock = mockAdminApis(
    () => current,
    () => credentialConfiguration(1, []),
    (input, init) => {
      if (String(input).includes("/provider-endpoints") && init?.method === "POST") {
        current = configuration(2, [
          endpoint({
            name: "本地 Claude",
            provider_kind: "claude",
            base_url: "http://127.0.0.1:8080",
            protocol_dialect: "anthropic_messages",
          }),
        ]);
        return jsonResponse(current);
      }
      return null;
    },
  );

  renderManagement(["/providers?kind=claude&editor=new"]);

  fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "本地 Claude" } });
  fireEvent.change(screen.getByLabelText("Base URL"), {
    target: { value: "http://127.0.0.1:8080" },
  });
  expect(screen.queryByRole("switch", { name: "允许普通 HTTP" })).not.toBeInTheDocument();
  expect(screen.queryByRole("switch", { name: "允许内网地址" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("http://127.0.0.1:8080")).toBeInTheDocument();
  const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(JSON.parse(String(post?.[1]?.body))).toEqual({
    expected_revision: 1,
    name: "本地 Claude",
    provider_kind: "claude",
    base_url: "http://127.0.0.1:8080",
    protocol_dialect: "anthropic_messages",
    upstream_protocol_dialect: null,
    enabled: true,
  });
});

test("creates a Grok endpoint with the official xAI defaults", async () => {
  let current = configuration(1, []);
  const fetchMock = mockAdminApis(
    () => current,
    () => credentialConfiguration(1, []),
    (input, init) => {
      if (String(input).includes("/provider-endpoints") && init?.method === "POST") {
        current = configuration(2, [
          endpoint({
            name: "Grok Primary",
            provider_kind: "grok",
            base_url: "https://api.x.ai/v1",
          }),
        ]);
        return jsonResponse(current);
      }
      return null;
    },
  );

  renderManagement(["/providers?kind=grok&editor=new"]);

  fireEvent.change(await screen.findByLabelText("名称"), {
    target: { value: "Grok Primary" },
  });
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://api.x.ai/v1");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("https://api.x.ai/v1")).toBeInTheDocument();
  const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(JSON.parse(String(post?.[1]?.body))).toMatchObject({
    provider_kind: "grok",
    base_url: "https://api.x.ai/v1",
    protocol_dialect: "openai_responses",
  });
});

test("creates a Codex endpoint for the OpenAI Images protocol", async () => {
  let current = configuration(1, []);
  const fetchMock = mockAdminApis(
    () => current,
    () => credentialConfiguration(1, []),
    (input, init) => {
      if (String(input).includes("/provider-endpoints") && init?.method === "POST") {
        current = configuration(2, [
          endpoint({
            name: "OpenAI Images",
            protocol_dialect: "openai_images",
          }),
        ]);
        return jsonResponse(current);
      }
      return null;
    },
  );

  renderManagement(["/providers?kind=codex&editor=new"]);

  fireEvent.change(await screen.findByLabelText("名称"), {
    target: { value: "OpenAI Images" },
  });
  fireEvent.change(screen.getByLabelText("接受协议"), {
    target: { value: "openai_images" },
  });
  expect(screen.getByLabelText("内部转换协议（可选）")).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => {
    expect(fetchMock.mock.calls.some(([, init]) => init?.method === "POST")).toBe(true);
  });
  const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(JSON.parse(String(post?.[1]?.body))).toMatchObject({
    provider_kind: "codex",
    protocol_dialect: "openai_images",
    upstream_protocol_dialect: null,
  });
});

test("refetches after a revision conflict without discarding the endpoint draft", async () => {
  let getCount = 0;
  mockAdminApis(
    () => {
      getCount += 1;
      return configuration(getCount === 1 ? 1 : 2, []);
    },
    () => credentialConfiguration(1, []),
    (_input, init) => {
      if (init?.method === "POST") {
        return new Response(
          JSON.stringify({ error: { code: "revision_conflict", message: "configuration changed" } }),
          { status: 409, headers: { "Content-Type": "application/json" } },
        );
      }
      return null;
    },
  );

  renderManagement(["/providers?editor=new"]);
  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "保留的 Endpoint 草稿" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(screen.getByDisplayValue("保留的 Endpoint 草稿")).toBeInTheDocument();
  expect(await screen.findByText(/配置已发生变化/)).toBeInTheDocument();
  expect(getCount).toBeGreaterThan(1);
});

test("preserves the draft but blocks overwrite when the endpoint version changed", async () => {
  let getCount = 0;
  const fetchMock = mockAdminApis(
    () => {
      getCount += 1;
      return configuration(
        getCount === 1 ? 1 : 2,
        [
          endpoint({
            name: getCount === 1 ? "Codex Primary" : "Codex Renamed Elsewhere",
            config_version: getCount === 1 ? 1 : 2,
          }),
        ],
      );
    },
    () => credentialConfiguration(1, []),
    (_input, init) => {
      if (init?.method === "PATCH") {
        return new Response(
          JSON.stringify({
            error: { code: "revision_conflict", message: "configuration changed" },
          }),
          { status: 409, headers: { "Content-Type": "application/json" } },
        );
      }
      return null;
    },
  );

  renderManagement(["/providers?editor=1e96eff2-7b3f-4974-b013-8fd2f44c8c1f"]);
  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "Local Draft" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(screen.getByDisplayValue("Local Draft")).toBeInTheDocument();
  expect(await screen.findByText(/已被其他操作修改/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  const patches = fetchMock.mock.calls.filter(([, init]) => init?.method === "PATCH");
  expect(JSON.parse(String(patches[0]?.[1]?.body))).toMatchObject({
    expected_revision: 1,
    expected_config_version: 1,
  });
  expect(patches).toHaveLength(1);
});

test("preserves the draft and blocks saving when the endpoint was deleted", async () => {
  let getCount = 0;
  mockAdminApis(
    () => {
      getCount += 1;
      return configuration(getCount === 1 ? 1 : 2, getCount === 1 ? [endpoint()] : []);
    },
    () => credentialConfiguration(1, []),
    (_input, init) => {
      if (init?.method === "PATCH") {
        return new Response(
          JSON.stringify({
            error: { code: "revision_conflict", message: "configuration changed" },
          }),
          { status: 409, headers: { "Content-Type": "application/json" } },
        );
      }
      return null;
    },
  );

  renderManagement(["/providers?editor=1e96eff2-7b3f-4974-b013-8fd2f44c8c1f"]);
  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "Retained Draft" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText(/已从最新配置中删除/)).toBeInTheDocument();
  expect(screen.getByDisplayValue("Retained Draft")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
});
