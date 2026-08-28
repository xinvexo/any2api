import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { ProxyProfile } from "../api/proxy-contracts";
import { ProxyEditor } from "./ProxyEditor";

test("locks a rejected proxy draft when the refreshed source version changes", async () => {
  const onSubmit = vi.fn().mockRejectedValue(new Error("configuration changed"));
  const onClose = vi.fn();
  const rendered = render(
    <ProxyEditor
      profile={profile(1)}
      isGlobal={false}
      configRevision={1}
      pending={false}
      error={null}
      onSubmit={onSubmit}
      onClose={onClose}
    />,
  );

  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "Edited proxy" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledOnce());
  expect(onSubmit.mock.calls[0]?.[0].input.expectedRevision).toBe(1);

  rendered.rerender(
    <ProxyEditor
      profile={profile(2)}
      isGlobal={false}
      configRevision={2}
      pending={false}
      error={new Error("configuration changed")}
      onSubmit={onSubmit}
      onClose={onClose}
    />,
  );

  expect(screen.getByText(/当前草稿基于旧版本，不能直接保存/)).toBeInTheDocument();
  expect(screen.getByLabelText("名称")).toHaveValue("Edited proxy");
  expect(screen.getByLabelText("名称")).toBeEnabled();
  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "Revised draft" } });
  expect(screen.getByLabelText("名称")).toHaveValue("Revised draft");
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledOnce();
});

function profile(configVersion: number): ProxyProfile {
  return {
    id: "proxy-1",
    name: "Proxy",
    kind: "http",
    host: "proxy.example.com",
    port: 8080,
    username: null,
    passwordConfigured: false,
    authenticationVersion: 1,
    enabled: true,
    builtIn: false,
    configVersion,
  };
}
