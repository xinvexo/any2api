import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { useLocation } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import {
  credential,
  credentialConfiguration,
  credentialId,
  credentialTestResult,
  endpoint,
  jsonResponse,
  proxyConfiguration,
  renderCredentialManagement,
} from "./ProviderCredentialManagement.test-support";
import { clearNotifications, getNotifications } from "@/shared/notifications";

afterEach(() => {
  clearNotifications();
  vi.restoreAllMocks();
});

test("creates a credential without retaining its secret in application caches", async () => {
  const secret = "sk-browser-secret-value";
  let credentials = credentialConfiguration(2, []);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (path.endsWith(`/provider-credentials/${credentialId}/test`)) {
      return jsonResponse(credentialTestResult());
    }
    if (path.endsWith(`/provider-credentials/${credentialId}/models`) && init?.method === "PUT") {
      credentials = credentialConfiguration(4, [
        credential({ config_version: 2, models: ["gpt-5.1-codex"] }),
      ]);
      return jsonResponse(credentials);
    }
    if (path.endsWith(`/provider-endpoints/${endpoint.id}/credentials`) && init?.method === "POST") {
      credentials = credentialConfiguration(3, [credential()]);
      return jsonResponse(credentials);
    }
    return jsonResponse(credentials);
  });
  const { client } = renderManagement([`/providers/codex?keys=${endpoint.id}&credential=new`]);

  const proxySelect = await screen.findByRole("combobox", { name: "出口代理" });
  fireEvent.click(proxySelect);
  expect(await screen.findByRole("option", { name: "DIRECT" })).toBeInTheDocument();
  expect(screen.getByRole("option", { name: "香港代理" })).toBeInTheDocument();
  fireEvent.keyDown(proxySelect, { key: "Escape" });
  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "Primary Key" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: secret } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  const model = await screen.findByRole("checkbox", { name: "gpt-5.1-codex" });
  expect(getNotifications()).toEqual([
    expect.objectContaining({ message: "已创建「Primary Key」", tone: "success" }),
  ]);
  const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
  expect(JSON.parse(String(post?.[1]?.body))).toMatchObject({
    api_key: secret,
    requests_per_minute: null,
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
  });
  expect(screen.queryByLabelText("本次保存的 API Key")).not.toBeInTheDocument();
  expect(screen.getByTestId("location")).not.toHaveTextContent(secret);
  expect(JSON.stringify(client.getQueryCache().getAll().map((query) => query.state.data))).not.toContain(secret);
  expect(JSON.stringify(client.getMutationCache().getAll())).not.toContain(secret);

  fireEvent.click(model);
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(screen.getByTestId("location")).toHaveTextContent("/providers"));
  expect(getNotifications().map((item) => item.message)).toEqual([
    "已保存「Primary Key」的模型选择",
    "已创建「Primary Key」",
  ]);
  const modelPut = fetchMock.mock.calls.find(
    ([input, init]) => String(input).endsWith(`/provider-credentials/${credentialId}/models`) && init?.method === "PUT",
  );
  expect(JSON.parse(String(modelPut?.[1]?.body))).toEqual({
    expected_revision: 3,
    expected_config_version: 1,
    models: ["gpt-5.1-codex"],
  });
  expect(document.body.innerHTML).not.toContain(secret);
  expect(screen.getByTestId("location")).not.toHaveTextContent(secret);
});

test("edits credential metadata without sending the secret", async () => {
  let credentials = credentialConfiguration(3, [credential()]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (init?.method === "PATCH") {
      credentials = credentialConfiguration(4, [
        credential({ label: "Edited", requests_per_minute: 12, config_version: 2 }),
      ]);
    }
    return jsonResponse(credentials);
  });
  renderManagement([`/providers/codex?keys=${endpoint.id}&credential=${credentialId}`]);

  const name = await screen.findByLabelText("名称");
  fireEvent.change(name, { target: { value: "Edited" } });
  fireEvent.change(screen.getByLabelText("RPM 限制"), { target: { value: "12" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await screen.findByText("Edited");
  const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
  const body = JSON.parse(String(patch?.[1]?.body)) as Record<string, unknown>;
  expect(body).toMatchObject({
    expected_revision: 3,
    expected_config_version: 1,
    label: "Edited",
    requests_per_minute: 12,
  });
  expect(body).not.toHaveProperty("api_key");
  expect(getNotifications()).toEqual([
    expect.objectContaining({ message: "已保存「Edited」", tone: "success" }),
  ]);
});

test("uses icon-only credential actions and toggles a credential inline", async () => {
  let credentials = credentialConfiguration(3, [credential()]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (init?.method === "PATCH") {
      credentials = credentialConfiguration(4, [
        credential({ enabled: false, config_version: 2 }),
      ]);
    }
    return jsonResponse(credentials);
  });
  renderManagement([`/providers/codex?keys=${endpoint.id}`]);

  const models = await screen.findByRole("button", {
    name: "配置 Primary Key 的模型",
  });
  const edit = screen.getByRole("button", { name: "编辑 Primary Key" });
  const remove = screen.getByRole("button", { name: "删除 Primary Key" });
  const disable = screen.getByRole("button", { name: "停用 Primary Key" });

  expect(models).not.toHaveTextContent("模型");
  expect(edit).not.toHaveTextContent("编辑");
  expect(remove).not.toHaveTextContent("删除");
  expect(models).toHaveAttribute("title", "配置 Primary Key 的模型");

  fireEvent.click(disable);

  expect(await screen.findByRole("button", { name: "启用 Primary Key" })).toBeInTheDocument();
  await waitFor(() => {
    const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
    expect(JSON.parse(String(patch?.[1]?.body))).toEqual({
      expected_revision: 3,
      expected_config_version: 1,
      label: "Primary Key",
      proxy_profile_id: "00000000-0000-0000-0000-000000000000",
      requests_per_minute: 4,
      enabled: false,
    });
  });
});

test("opens a credential model picker and loads the current upstream catalog", async () => {
  const credentials = credentialConfiguration(3, [credential()]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const path = String(input);
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (path.endsWith(`/provider-credentials/${credentialId}/test`)) {
      return jsonResponse({
        config_revision: 3,
        provider_endpoint_config_version: 1,
        credential_config_version: 1,
        credential_generation: 1,
        secret_version: 1,
        proxy_config_version: 1,
        credential_id: credentialId,
        provider_endpoint_id: endpoint.id,
        proxy_id: "f0335fed-e5a9-4081-966b-37efe4a109a8",
        reachable: true,
        accepted: true,
        catalog_valid: true,
        status_code: 200,
        latency_ms: 18,
        auth_error_cleared: true,
        error_stage: null,
        failure_scope: null,
        models: ["gpt-5.1-codex"],
      });
    }
    return jsonResponse(credentials);
  });
  renderManagement([`/providers/codex?keys=${endpoint.id}`]);

  fireEvent.click(await screen.findByRole("button", { name: "配置 Primary Key 的模型" }));

  expect(await screen.findByRole("checkbox", { name: "gpt-5.1-codex" })).toBeInTheDocument();
  expect(screen.getByText(/已读取 1 个模型/)).toBeInTheDocument();
  expect(getNotifications()).toHaveLength(0);
  fireEvent.click(screen.getByRole("button", { name: "重新拉取" }));
  await waitFor(() => {
    expect(
      fetchMock.mock.calls.filter(([input]) => String(input).endsWith("/test")),
    ).toHaveLength(2);
  });
  expect(getNotifications()).toEqual([
    expect.objectContaining({ message: "已读取「Primary Key」的上游模型", tone: "success" }),
  ]);
  const request = fetchMock.mock.calls.find(([input]) => String(input).endsWith("/test"));
  expect(request?.[1]?.method).toBe("POST");
  expect(request?.[1]?.body).toBeUndefined();
});

test("keeps in-flight model discovery across an unrelated config revision", async () => {
  let credentials = credentialConfiguration(3, [credential()]);
  const discovery = deferred<Response>();
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const path = String(input);
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (path.endsWith(`/provider-credentials/${credentialId}/test`)) {
      return discovery.promise;
    }
    return jsonResponse(credentials);
  });
  const { client } = renderManagement([`/providers/codex?keys=${endpoint.id}`]);

  fireEvent.click(await screen.findByRole("button", { name: "配置 Primary Key 的模型" }));
  await waitFor(() => {
    expect(fetchMock.mock.calls.filter(([input]) => String(input).endsWith("/test"))).toHaveLength(1);
  });

  credentials = credentialConfiguration(4, [credential()]);
  await act(async () => {
    await client.refetchQueries({
      queryKey: ["provider-endpoints", "credentials", endpoint.id],
      type: "active",
    });
  });

  expect(screen.getByText("正在读取上游模型")).toBeInTheDocument();
  await act(async () => {
    discovery.resolve(jsonResponse(credentialTestResult()));
    await discovery.promise;
  });

  expect(await screen.findByRole("checkbox", { name: "gpt-5.1-codex" })).toBeInTheDocument();
  expect(screen.getByText(/已读取 2 个模型/)).toBeInTheDocument();
});

test("saves a manually entered model when discovery returns an empty catalog", async () => {
  let credentials = credentialConfiguration(3, [credential()]);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (path.endsWith(`/provider-credentials/${credentialId}/test`)) {
      return jsonResponse({ ...credentialTestResult(), models: [] });
    }
    if (path.endsWith(`/provider-credentials/${credentialId}/models`) && init?.method === "PUT") {
      credentials = credentialConfiguration(4, [
        credential({ config_version: 2, models: ["gpt-5.6-sol"] }),
      ]);
      return jsonResponse(credentials);
    }
    return jsonResponse(credentials);
  });
  renderManagement([`/providers/codex?keys=${endpoint.id}`]);

  fireEvent.click(await screen.findByRole("button", { name: "配置 Primary Key 的模型" }));
  expect(await screen.findByText(/上游返回了空模型列表/)).toBeInTheDocument();

  fireEvent.change(screen.getByLabelText("手动添加模型"), {
    target: { value: "gpt-5.6-sol" },
  });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => {
    const modelPut = fetchMock.mock.calls.find(
      ([input, init]) =>
        String(input).endsWith(`/provider-credentials/${credentialId}/models`) &&
        init?.method === "PUT",
    );
    expect(JSON.parse(String(modelPut?.[1]?.body))).toEqual({
      expected_revision: 3,
      expected_config_version: 1,
      models: ["gpt-5.6-sol"],
    });
  });
});

test("keeps model save failures inside the editor without an unhandled rejection", async () => {
  const credentials = credentialConfiguration(3, [credential()]);
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path === "/api/admin/proxies") {
      return jsonResponse(proxyConfiguration());
    }
    if (path.endsWith(`/provider-credentials/${credentialId}/test`)) {
      return jsonResponse(credentialTestResult());
    }
    if (path.endsWith(`/provider-credentials/${credentialId}/models`) && init?.method === "PUT") {
      return new Response(
        JSON.stringify({ code: "revision_conflict", message: "configuration changed" }),
        {
          status: 409,
          headers: { "Content-Type": "application/json" },
        },
      );
    }
    return jsonResponse(credentials);
  });
  renderManagement([`/providers/codex?keys=${endpoint.id}`]);

  fireEvent.click(await screen.findByRole("button", { name: "配置 Primary Key 的模型" }));
  fireEvent.click(await screen.findByRole("button", { name: "保存" }));

  expect(await screen.findByRole("alert")).toBeInTheDocument();
  expect(screen.getByTestId("location")).toHaveTextContent("action=models");
  expect(getNotifications()).toHaveLength(0);
});

function renderManagement(initialEntries: string[]) {
  return renderCredentialManagement(initialEntries, <LocationProbe />);
}

function LocationProbe() {
  const location = useLocation();
  return <span data-testid="location" hidden>{`${location.pathname}${location.search}`}</span>;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}
