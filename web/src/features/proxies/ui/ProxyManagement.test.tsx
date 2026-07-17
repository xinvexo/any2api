import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { ProxyManagement } from "./ProxyManagement";

const direct = {
  id: "00000000-0000-0000-0000-000000000000",
  name: "DIRECT",
  kind: "direct",
  host: null,
  port: null,
  enabled: true,
  built_in: true,
  config_version: 1,
};

afterEach(() => vi.restoreAllMocks());

test("renders DIRECT and explains global inheritance", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(configuration(1, [direct])));

  renderManagement();

  expect((await screen.findAllByText("DIRECT")).length).toBeGreaterThan(1);
  expect(screen.getByText(/Credential 绑定 DIRECT 时会继承此出口/)).toBeInTheDocument();
  expect(screen.getByText("尚未添加自定义代理。新代理会独立保存，不会改变当前全局出口。")).toBeInTheDocument();
});

test("creates a SOCKS5 proxy with the visible configuration revision", async () => {
  let current = configuration(1, [direct]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "POST") {
      current = configuration(2, [
        direct,
        {
          id: "a81bf8f8-8fb4-45f0-926d-1cfda84884f5",
          name: "香港出口",
          kind: "socks5",
          host: "hk.example.com",
          port: 1080,
          enabled: true,
          built_in: false,
          config_version: 1,
        },
      ]);
    }
    return jsonResponse(current);
  });

  renderManagement(["/proxies?editor=new"]);

  fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "香港出口" } });
  fireEvent.change(screen.getByLabelText("类型"), { target: { value: "socks5" } });
  fireEvent.change(screen.getByLabelText("主机"), { target: { value: "hk.example.com" } });
  fireEvent.change(screen.getByLabelText("端口"), { target: { value: "1080" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("hk.example.com:1080")).toBeInTheDocument();
  const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(post).toBeDefined();
  expect(JSON.parse(String(post?.[1]?.body))).toEqual({
    expected_revision: 1,
    name: "香港出口",
    kind: "socks5",
    host: "hk.example.com",
    port: 1080,
    enabled: true,
  });
});

test("does not render an editor for a DIRECT deep link", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(configuration(1, [direct])));

  renderManagement([`/proxies?editor=${direct.id}`]);

  expect(await screen.findByText("DIRECT 不可编辑")).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "编辑代理" })).not.toBeInTheDocument();
});

test("refetches the revision after a write conflict without discarding the editor", async () => {
  let getCount = 0;
  vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "POST") {
      return new Response(
        JSON.stringify({
          error: { code: "revision_conflict", message: "configuration changed" },
        }),
        { status: 409, headers: { "Content-Type": "application/json" } },
      );
    }
    getCount += 1;
    return jsonResponse(configuration(getCount === 1 ? 1 : 2, [direct]));
  });

  renderManagement(["/proxies?editor=new"]);
  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "保留的草稿" } });
  fireEvent.change(screen.getByLabelText("主机"), { target: { value: "proxy.example.com" } });
  fireEvent.change(screen.getByLabelText("端口"), { target: { value: "8080" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("2")).toBeInTheDocument();
  expect(screen.getByDisplayValue("保留的草稿")).toBeInTheDocument();
  expect(getCount).toBeGreaterThan(1);
});

function renderManagement(initialEntries = ["/proxies"]) {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries}>
        <ProxyManagement />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function configuration(revision: number, items: unknown[]) {
  return {
    config_revision: revision,
    global_proxy_id: direct.id,
    items,
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
