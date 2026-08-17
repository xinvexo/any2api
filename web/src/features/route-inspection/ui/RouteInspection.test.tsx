import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { RouteInspection } from "./RouteInspection";

afterEach(() => vi.restoreAllMocks());

test("renders model cards and filters by exact model name and finite route status", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(response()));
  renderInspection();

  expect(await screen.findByText("available-model")).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "路由检查" })).not.toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "精确模型搜索" }).closest("header"))
    .toHaveClass("min-h-8", "border-b", "pb-3");
  expect(screen.getByRole("listitem", { name: "available-model 路由" })).toHaveClass(
    "rounded-[8px]",
  );
  expect(screen.getByText("未发布")).toBeInTheDocument();

  fireEvent.change(screen.getByRole("textbox", { name: "精确模型搜索" }), {
    target: { value: "available" },
  });
  expect(screen.queryByText("available-model")).not.toBeInTheDocument();
  expect(screen.getByText("没有匹配的路由")).toBeInTheDocument();

  fireEvent.change(screen.getByRole("textbox", { name: "精确模型搜索" }), {
    target: { value: "" },
  });
  fireEvent.click(screen.getByRole("combobox", { name: "状态筛选" }));
  fireEvent.click(screen.getByRole("option", { name: "无启用候选" }));
  await waitFor(() => {
    expect(screen.getByText("disabled-model")).toBeInTheDocument();
    expect(screen.queryByText("available-model")).not.toBeInTheDocument();
  });
});

test("keeps the filter toolbar in place while the first request is pending", () => {
  vi.spyOn(globalThis, "fetch").mockImplementation(() => new Promise<Response>(() => {}));
  renderInspection();

  expect(screen.getByText("正在读取路由配置")).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "精确模型搜索" })).toBeDisabled();
  expect(screen.getByRole("combobox", { name: "状态筛选" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "刷新" })).toBeDisabled();
  expect(screen.queryByRole("heading", { name: "路由检查" })).not.toBeInTheDocument();
});

function renderInspection() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <RouteInspection />
    </QueryClientProvider>,
  );
}

function response() {
  return {
    config_revision: 3,
    items: [
      item("available-model", "available"),
      item("disabled-model", "no_enabled_candidate", false),
    ],
  };
}

function item(
  publicModel: string,
  status: string,
  published = true,
) {
  return {
    public_model: publicModel,
    ingress_protocol: "openai_responses",
    published,
    status,
    operations: [
      {
        operation: "responses",
        candidate_groups:
          status === "no_enabled_candidate"
            ? []
            : [
                {
                  provider_kind: "codex",
                  provider_endpoint_id: "5ba99aba-62e2-44d2-b98d-5ef6906c479a",
                  provider_endpoint_name: "Codex Primary",
                  upstream_protocol_dialect: "openai_responses",
                  enabled_candidate_count: 1,
                },
              ],
      },
    ],
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
