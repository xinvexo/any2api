import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { RequestLogFilterOptions } from "../api/request-log-filter-contracts";
import { RequestLogFilterBar } from "./RequestLogFilterBar";

test("emits only the supported exact request log filters", () => {
  const onChange = vi.fn();
  render(<RequestLogFilterBar filters={{}} options={filterOptions()} onChange={onChange} />);

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
  render(<RequestLogFilterBar filters={{}} options={filterOptions()} onChange={() => {}} />);

  expect(screen.queryByLabelText("Request ID")).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "操作" })).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "上游凭据" })).not.toBeInTheDocument();
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
