import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { ProviderManagement } from "./ProviderManagement";

afterEach(() => vi.restoreAllMocks());

test("shows the empty Provider state and network safety policy", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(configuration(1, [])));

  renderManagement();

  expect(await screen.findByText("还没有 Provider Endpoint")).toBeInTheDocument();
  expect(screen.getByText(/默认只允许公网 HTTPS/)).toBeInTheDocument();
});

test("creates a Claude private HTTP endpoint with separate authorizations", async () => {
  let current = configuration(1, []);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "POST") {
      current = configuration(2, [
        endpoint({
          name: "本地 Claude",
          provider_kind: "claude",
          base_url: "http://127.0.0.1:8080",
          protocol_dialect: "anthropic_messages",
          allow_insecure_http: true,
          allow_private_network: true,
        }),
      ]);
    }
    return jsonResponse(current);
  });

  renderManagement(["/providers?editor=new"]);

  fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "本地 Claude" } });
  fireEvent.change(screen.getByLabelText("Provider"), { target: { value: "claude" } });
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: "http://127.0.0.1:8080" } });
  fireEvent.click(screen.getByLabelText("允许普通 HTTP"));
  fireEvent.click(screen.getByLabelText("允许内网地址"));
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("http://127.0.0.1:8080")).toBeInTheDocument();
  const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(JSON.parse(String(post?.[1]?.body))).toEqual({
    expected_revision: 1,
    name: "本地 Claude",
    provider_kind: "claude",
    base_url: "http://127.0.0.1:8080",
    protocol_dialect: "anthropic_messages",
    allow_insecure_http: true,
    allow_private_network: true,
    enabled: true,
  });
});

test("refetches after a revision conflict without discarding the endpoint draft", async () => {
  let getCount = 0;
  vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "POST") {
      return new Response(
        JSON.stringify({ error: { code: "revision_conflict", message: "configuration changed" } }),
        { status: 409, headers: { "Content-Type": "application/json" } },
      );
    }
    getCount += 1;
    return jsonResponse(configuration(getCount === 1 ? 1 : 2, []));
  });

  renderManagement(["/providers?editor=new"]);
  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "保留的 Endpoint 草稿" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("2")).toBeInTheDocument();
  expect(screen.getByDisplayValue("保留的 Endpoint 草稿")).toBeInTheDocument();
  expect(getCount).toBeGreaterThan(1);
});

test("preserves the draft but blocks overwrite when the endpoint version changed", async () => {
  let getCount = 0;
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      return new Response(JSON.stringify({
        error: { code: "revision_conflict", message: "configuration changed" },
      }), {
        status: 409,
        headers: { "Content-Type": "application/json" },
      });
    }
    getCount += 1;
    return jsonResponse(
      configuration(getCount === 1 ? 1 : 2, [
        endpoint({
          name: getCount === 1 ? "Original" : "Remote Edit",
          config_version: getCount === 1 ? 1 : 2,
        }),
      ]),
    );
  });

  renderManagement(["/providers?editor=1e96eff2-7b3f-4974-b013-8fd2f44c8c1f"]);
  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "Local Draft" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("2")).toBeInTheDocument();
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
  vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "PATCH") {
      return new Response(JSON.stringify({
        error: { code: "revision_conflict", message: "configuration changed" },
      }), {
        status: 409,
        headers: { "Content-Type": "application/json" },
      });
    }
    getCount += 1;
    return jsonResponse(
      configuration(getCount === 1 ? 1 : 2, getCount === 1 ? [endpoint()] : []),
    );
  });

  renderManagement(["/providers?editor=1e96eff2-7b3f-4974-b013-8fd2f44c8c1f"]);
  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "Retained Draft" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText(/已从最新配置中删除/)).toBeInTheDocument();
  expect(screen.getByDisplayValue("Retained Draft")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
});

function renderManagement(initialEntries = ["/providers"]) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries}>
        <ProviderManagement />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function configuration(revision: number, items: unknown[]) {
  return { config_revision: revision, items };
}

function endpoint(overrides: Record<string, unknown> = {}) {
  return {
    id: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
    name: "Codex Primary",
    provider_kind: "codex",
    base_url: "https://api.example.com/v1",
    protocol_dialect: "openai_responses",
    allow_insecure_http: false,
    allow_private_network: false,
    enabled: true,
    config_version: 1,
    ...overrides,
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
