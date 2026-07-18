import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation, useNavigate } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { ModelRouteManagement } from "./ModelRouteManagement";

afterEach(() => vi.restoreAllMocks());

test("shows the empty route state", async () => {
  mockConfigurationFetch(routeConfiguration(1, []), providerConfiguration([endpoint()]));

  renderManagement();

  expect(await screen.findByText("还没有模型路由")).toBeInTheDocument();
});

test("creates a route without client-generated target ids", async () => {
  let routes = routeConfiguration(1, []);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.includes("/api/admin/model-routes") && init?.method === "POST") {
      routes = routeConfiguration(2, [route({ public_model: "codex-main" })]);
      return jsonResponse(routes);
    }
    if (path.includes("/api/admin/model-routes")) {
      return jsonResponse(routes);
    }
    return jsonResponse(providerConfiguration([endpoint()]));
  });

  renderManagement(["/routes?editor=new"]);

  fireEvent.change(await screen.findByLabelText("上游模型 1"), {
    target: { value: "gpt-5.1-codex" },
  });
  expect(screen.getByLabelText("公开模型名")).toHaveValue("gpt-5.1-codex");
  fireEvent.change(screen.getByLabelText("公开模型名"), {
    target: { value: "codex-main" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(await screen.findByText("codex-main")).toBeInTheDocument();
  const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(JSON.parse(String(post?.[1]?.body))).toEqual({
    expected_revision: 1,
    public_model: "codex-main",
    ingress_protocol: "openai_responses",
    fallback_on_saturation: null,
    enabled: true,
    targets: [
      {
        provider_endpoint_id: endpointId,
        upstream_model: "gpt-5.1-codex",
        fallback_tier: 0,
        enabled: true,
      },
    ],
  });
});

test("preserves a target id when only routing policy changes", async () => {
  let routes = routeConfiguration(1, [route()]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.includes("/api/admin/model-routes") && init?.method === "PATCH") {
      routes = routeConfiguration(2, [route({ config_version: 2 })]);
      return jsonResponse(routes);
    }
    if (path.includes("/api/admin/model-routes")) {
      return jsonResponse(routes);
    }
    return jsonResponse(providerConfiguration([endpoint()]));
  });

  renderManagement([`/routes?editor=${routeId}`]);

  fireEvent.change(await screen.findByLabelText("Fallback tier 1"), {
    target: { value: "2" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() =>
    expect(fetchMock.mock.calls.some(([, init]) => init?.method === "PATCH")).toBe(true),
  );
  const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
  expect(JSON.parse(String(patch?.[1]?.body))).toMatchObject({
    expected_revision: 1,
    expected_config_version: 1,
    targets: [{ id: targetId, fallback_tier: 2 }],
  });
});

test("omits the old target id when endpoint identity changes", async () => {
  let routes = routeConfiguration(1, [route()]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.includes("/api/admin/model-routes") && init?.method === "PATCH") {
      routes = routeConfiguration(2, [
        route({
          config_version: 2,
          targets: [target({ id: replacementTargetId, provider_endpoint_id: secondEndpointId })],
        }),
      ]);
      return jsonResponse(routes);
    }
    if (path.includes("/api/admin/model-routes")) {
      return jsonResponse(routes);
    }
    return jsonResponse(providerConfiguration([
      endpoint(),
      endpoint({ id: secondEndpointId, name: "Codex Backup" }),
    ]));
  });

  renderManagement([`/routes?editor=${routeId}`]);

  fireEvent.change(await screen.findByLabelText("Provider Endpoint 1"), {
    target: { value: secondEndpointId },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() =>
    expect(fetchMock.mock.calls.some(([, init]) => init?.method === "PATCH")).toBe(true),
  );
  const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
  expect(JSON.parse(String(patch?.[1]?.body)).targets[0]).toEqual({
    provider_endpoint_id: secondEndpointId,
    upstream_model: "gpt-5.1-codex",
    fallback_tier: 0,
    enabled: true,
  });
});

test("keeps a valid deep link pending until a stale route cache refreshes", async () => {
  const refreshedRoutes = deferredResponse();
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    if (String(input).includes("/api/admin/model-routes")) {
      return refreshedRoutes.promise;
    }
    return jsonResponse(providerConfiguration([endpoint()]));
  });

  renderManagement([`/routes?editor=${routeId}`], (client) => {
    client.setQueryData(["model-routes", "list"], {
      configRevision: 1,
      items: [],
    });
  });

  expect(await screen.findByText("正在读取模型路由")).toBeInTheDocument();
  refreshedRoutes.resolve(jsonResponse(routeConfiguration(2, [route()])));

  expect(await screen.findByDisplayValue("codex-main")).toBeInTheDocument();
  expect(screen.queryByText("模型路由不存在")).not.toBeInTheDocument();
});

test("fills a new target when endpoints arrive after an empty cached configuration", async () => {
  const refreshedEndpoints = deferredResponse();
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    if (String(input).includes("/api/admin/model-routes")) {
      return jsonResponse(routeConfiguration(1, []));
    }
    return refreshedEndpoints.promise;
  });

  renderManagement(["/routes?editor=new"], (client) => {
    client.setQueryData(["model-routes", "list"], {
      configRevision: 1,
      items: [],
    });
    client.setQueryData(["provider-endpoints", "list"], {
      configRevision: 1,
      items: [],
    });
  });

  expect(await screen.findByLabelText("Provider Endpoint 1")).toHaveValue("");
  refreshedEndpoints.resolve(jsonResponse(providerConfiguration([endpoint()])));

  await waitFor(() =>
    expect(screen.getByLabelText("Provider Endpoint 1")).toHaveValue(endpointId),
  );
});

test("does not close a newer location after an old save completes", async () => {
  let routes = routeConfiguration(1, []);
  const saved = deferredResponse();
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.includes("/api/admin/model-routes") && init?.method === "POST") {
      return saved.promise;
    }
    if (path.includes("/api/admin/model-routes")) {
      return jsonResponse(routes);
    }
    return jsonResponse(providerConfiguration([endpoint()]));
  });

  renderManagement(["/before", "/routes"], undefined, true);

  fireEvent.click(await screen.findByRole("button", { name: "新增路由" }));
  fireEvent.change(await screen.findByLabelText("上游模型 1"), {
    target: { value: "gpt-5.1-codex" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  fireEvent.click(screen.getByRole("button", { name: "测试返回" }));
  expect(screen.getByTestId("location")).toHaveTextContent("/routes");

  routes = routeConfiguration(2, [route({ public_model: "gpt-5.1-codex" })]);
  saved.resolve(jsonResponse(routes));
  expect(await screen.findByText("gpt-5.1-codex")).toBeInTheDocument();
  expect(screen.getByTestId("location")).toHaveTextContent("/routes");
});

test("shows an invalid deep link action below the desktop breakpoint", async () => {
  mockConfigurationFetch(routeConfiguration(1, []), providerConfiguration([endpoint()]));

  renderManagement([`/routes?editor=${routeId}`]);

  const message = await screen.findByText("模型路由不存在");
  expect(message.closest(".hidden")).toBeNull();
  expect(screen.getByRole("button", { name: "返回列表" })).toBeInTheDocument();
});

function renderManagement(
  initialEntries = ["/routes"],
  configureClient?: (client: QueryClient) => void,
  navigationControls = false,
) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  configureClient?.(client);
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries} initialIndex={initialEntries.length - 1}>
        <ModelRouteManagement />
        {navigationControls ? <NavigationControls /> : null}
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function NavigationControls() {
  const location = useLocation();
  const navigate = useNavigate();
  return (
    <>
      <button type="button" onClick={() => navigate(-1)}>测试返回</button>
      <output data-testid="location">{location.pathname}{location.search}</output>
    </>
  );
}

function mockConfigurationFetch(routes: unknown, providers: unknown) {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) =>
    jsonResponse(String(input).includes("model-routes") ? routes : providers),
  );
}

function routeConfiguration(revision: number, items: unknown[]) {
  return { config_revision: revision, items };
}

function providerConfiguration(items: unknown[]) {
  return { config_revision: 1, items };
}

function route(overrides: Record<string, unknown> = {}) {
  return {
    id: routeId,
    public_model: "codex-main",
    ingress_protocol: "openai_responses",
    fallback_on_saturation: null,
    enabled: true,
    config_version: 1,
    targets: [target()],
    ...overrides,
  };
}

function target(overrides: Record<string, unknown> = {}) {
  return {
    id: targetId,
    provider_endpoint_id: endpointId,
    upstream_model: "gpt-5.1-codex",
    fallback_tier: 0,
    enabled: true,
    ...overrides,
  };
}

function endpoint(overrides: Record<string, unknown> = {}) {
  return {
    id: endpointId,
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

function deferredResponse() {
  let resolve!: (response: Response) => void;
  const promise = new Promise<Response>((complete) => {
    resolve = complete;
  });
  return { promise, resolve };
}

const routeId = "f9937387-09ba-4d7a-ad08-2ab214aace86";
const targetId = "f78b62bc-13e7-45ce-9df3-11a067160db7";
const replacementTargetId = "6e04302b-6d8b-4c26-a8eb-34450bb18ac3";
const endpointId = "59b274af-f540-41d8-bf24-95ef07277502";
const secondEndpointId = "e970499a-b346-48a3-953d-20426333e6da";
