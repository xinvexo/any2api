import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { expect, test, vi } from "vitest";

import type { RequestLogFilterOptions } from "../api/request-log-filter-contracts";
import { RequestLogFilterBar } from "./RequestLogFilterBar";

test("emits exact outcome and tagged upstream filters", () => {
  const onChange = vi.fn();
  render(
    <MemoryRouter>
      <RequestLogFilterBar filters={{}} options={filterOptions()} onChange={onChange} />
    </MemoryRouter>,
  );

  fireEvent.click(screen.getByRole("combobox", { name: "结果" }));
  fireEvent.click(screen.getByRole("option", { name: "失败" }));
  expect(onChange).toHaveBeenLastCalledWith({ outcome: "failed" });

  fireEvent.click(screen.getByRole("combobox", { name: "上游凭据" }));
  fireEvent.click(screen.getByRole("option", { name: "OAuth · codex / Work" }));
  expect(onChange).toHaveBeenLastCalledWith({
    credentialId: undefined,
    oauthAccountId: "22222222-2222-4222-8222-222222222222",
  });
});

test("navigates an exact Request ID to the existing detail route", () => {
  const requestId = "11111111-1111-4111-8111-111111111111";
  render(
    <MemoryRouter initialEntries={["/logs"]}>
      <Routes>
        <Route
          path="/logs"
          element={
            <RequestLogFilterBar filters={{}} options={filterOptions()} onChange={() => {}} />
          }
        />
        <Route path="/logs/:requestId" element={<p>request detail</p>} />
      </Routes>
    </MemoryRouter>,
  );

  fireEvent.change(screen.getByLabelText("Request ID"), { target: { value: "invalid" } });
  fireEvent.click(screen.getByRole("button", { name: "定位" }));
  expect(screen.getByRole("alert")).toHaveTextContent("Request ID 无效");

  fireEvent.change(screen.getByLabelText("Request ID"), { target: { value: requestId } });
  fireEvent.click(screen.getByRole("button", { name: "定位" }));
  expect(screen.getByText("request detail")).toBeInTheDocument();
});

function filterOptions(): RequestLogFilterOptions {
  return {
    publicModels: ["gpt-test"],
    gatewayApiKeys: [],
    providerCredentials: [
      {
        id: "11111111-1111-4111-8111-111111111111",
        label: "Codex / Primary",
        deleted: false,
      },
    ],
    oauthAccounts: [
      {
        id: "22222222-2222-4222-8222-222222222222",
        label: "codex / Work",
        deleted: false,
      },
    ],
  };
}
