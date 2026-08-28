import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { GatewayApiKeyEditor } from "./GatewayApiKeyEditor";

test("locks a rejected gateway key draft when the refreshed source version changes", async () => {
  const onSubmit = vi.fn().mockRejectedValue(new Error("configuration changed"));
  const onClose = vi.fn();
  const rendered = render(
    <GatewayApiKeyEditor
      apiKey={apiKey(1)}
      configRevision={1}
      pending={false}
      error={null}
      onSubmit={onSubmit}
      onClose={onClose}
    />,
  );

  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "Edited key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledOnce());
  expect(onSubmit.mock.calls[0]?.[0]).toMatchObject({
    expectedRevision: 1,
    expectedConfigVersion: 1,
  });

  rendered.rerender(
    <GatewayApiKeyEditor
      apiKey={apiKey(2)}
      configRevision={2}
      pending={false}
      error={new Error("configuration changed")}
      onSubmit={onSubmit}
      onClose={onClose}
    />,
  );

  expect(screen.getByText(/当前草稿基于旧版本，不能直接保存/)).toBeInTheDocument();
  expect(screen.getByLabelText("名称")).toHaveValue("Edited key");
  expect(screen.getByLabelText("名称")).toBeEnabled();
  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "Revised draft" } });
  expect(screen.getByLabelText("名称")).toHaveValue("Revised draft");
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledOnce();
});

function apiKey(configVersion: number): GatewayApiKey {
  return {
    id: "key-1",
    name: "Gateway key",
    tokenPrefix: "sk-aaaaaaaaaaaaa",
    tokenVersion: 1,
    configVersion,
    requestsPerMinute: null,
    enabled: true,
    createdAt: "2026-08-28 12:00:00",
    lastUsedAt: null,
    usage: {
      totalRequests: 0,
      successfulRequests: 0,
      failedRequests: 0,
      windowMinutes: 2,
      windowSlots: Array.from({ length: 30 }, (_, index) => ({
        startedAtMs: index * 120_000,
        totalRequests: 0,
        successfulRequests: 0,
        failedRequests: 0,
      })),
    },
  };
}
