import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { AffinityManagement } from "./AffinityManagement";

afterEach(() => vi.restoreAllMocks());

test("shows redacted bindings and clears one credential", async () => {
  let runtime = affinityRuntime();
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "DELETE") {
      runtime = emptyRuntime();
      return jsonResponse({ cleared_count: 2 });
    }
    return jsonResponse(runtime);
  });

  renderManagement();

  expect(await screen.findByRole("heading", { name: "Credential 绑定分布" })).toBeInTheDocument();
  expect(screen.getByText("abcdefghijkl")).toBeInTheDocument();
  expect(screen.queryByText("private-session-id")).not.toBeInTheDocument();

  fireEvent.click(
    screen.getByRole("button", { name: "清除 Credential credential-1 的会话绑定" }),
  );

  await waitFor(() =>
    expect(
      fetchMock.mock.calls.some(
        ([input, init]) =>
          String(input).endsWith("/api/admin/affinity/credentials/credential-1") &&
          init?.method === "DELETE",
      ),
    ).toBe(true),
  );
  expect(await screen.findByText("当前没有会话绑定")).toBeInTheDocument();
});

test("clears all runtime bindings after confirmation", async () => {
  let runtime = affinityRuntime();
  vi.spyOn(window, "confirm").mockReturnValue(true);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (_input, init) => {
    if (init?.method === "DELETE") {
      runtime = emptyRuntime();
      return jsonResponse({ cleared_count: 2 });
    }
    return jsonResponse(runtime);
  });

  renderManagement();
  fireEvent.click(await screen.findByRole("button", { name: "清除全部" }));

  await waitFor(() =>
    expect(
      fetchMock.mock.calls.some(
        ([input, init]) =>
          String(input).endsWith("/api/admin/affinity") && init?.method === "DELETE",
      ),
    ).toBe(true),
  );
  expect(await screen.findByText("暂无可展示的绑定样本")).toBeInTheDocument();
});

function renderManagement() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <AffinityManagement />
    </QueryClientProvider>,
  );
}

function affinityRuntime() {
  return {
    config_revision: 5,
    soft_binding_count: 1,
    hard_binding_count: 1,
    creating_count: 0,
    credential_counts: [
      {
        credential_id: "credential-1",
        credential_label: "Primary",
        soft_bindings: 1,
        hard_bindings: 1,
      },
    ],
    bindings: [
      {
        kind: "hard",
        session_hash_prefix: "abcdefghijkl",
        credential_id: "credential-1",
        route_target_id: "target-1",
        upstream_model: "gpt-upstream",
        protocol_dialect: "openai_responses",
        expires_in_ms: 30_000,
      },
    ],
  };
}

function emptyRuntime() {
  return {
    config_revision: 5,
    soft_binding_count: 0,
    hard_binding_count: 0,
    creating_count: 0,
    credential_counts: [],
    bindings: [],
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
