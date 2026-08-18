import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { RequestLogFilterOptions } from "../api/request-log-filter-contracts";
import { RequestLogFilterBar } from "./RequestLogFilterBar";

test("emits only the supported exact request log filters", () => {
  const onChange = vi.fn();
  render(
    <RequestLogFilterBar
      filters={{}}
      options={filterOptions()}
      onChange={onChange}
      onRefresh={vi.fn()}
      refreshing={false}
    />,
  );

  fireEvent.click(screen.getByRole("combobox", { name: "结果" }));
  fireEvent.click(screen.getByRole("option", { name: "失败" }));
  expect(onChange).toHaveBeenLastCalledWith({ outcome: "failed" });

  fireEvent.click(screen.getByRole("combobox", { name: "公开模型" }));
  fireEvent.click(screen.getByRole("option", { name: "gpt-test" }));
  expect(onChange).toHaveBeenLastCalledWith({ publicModel: "gpt-test" });

  fireEvent.click(screen.getByRole("combobox", { name: "Gateway API Key" }));
  fireEvent.click(screen.getByRole("option", { name: "Desktop" }));
  expect(onChange).toHaveBeenLastCalledWith({
    gatewayApiKeyId: "11111111-1111-4111-8111-111111111111",
  });
});

test("omits manual request ID, operation, and upstream credential controls", () => {
  render(
    <RequestLogFilterBar
      filters={{}}
      options={filterOptions()}
      onChange={() => {}}
      onRefresh={() => {}}
      refreshing={false}
    />,
  );

  expect(screen.queryByLabelText("Request ID")).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "操作" })).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "上游凭据" })).not.toBeInTheDocument();
});

test("keeps mobile filters in a stable two-column grid with aligned actions", () => {
  const onRefresh = vi.fn();
  render(
    <RequestLogFilterBar
      filters={{}}
      options={filterOptions()}
      onChange={() => {}}
      onRefresh={onRefresh}
      refreshing={false}
    />,
  );

  expect(screen.getByLabelText("请求日志筛选")).toHaveClass(
    "grid",
    "w-full",
    "grid-cols-2",
    "sm:flex",
  );
  expect(
    screen.getAllByRole("combobox").every((control) =>
      control.classList.contains("w-full")
    ),
  ).toBe(true);
  expect(screen.getByRole("button", { name: "重置请求日志筛选" })).toHaveClass("h-8");
  fireEvent.click(screen.getByRole("button", { name: "刷新" }));
  expect(onRefresh).toHaveBeenCalledOnce();
});

function filterOptions(): RequestLogFilterOptions {
  return {
    publicModels: ["gpt-test"],
    gatewayApiKeys: [
      {
        id: "11111111-1111-4111-8111-111111111111",
        label: "Desktop",
        deleted: false,
      },
    ],
  };
}
