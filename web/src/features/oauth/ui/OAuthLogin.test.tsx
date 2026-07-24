import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type { OAuthStartResult } from "../api/oauth-contracts";
import { OAuthLoginDrawer } from "./OAuthLogin";

afterEach(() => vi.restoreAllMocks());

const session: OAuthStartResult = {
  provider: "codex",
  sessionId: "session-1",
  authorizationUrl: "https://auth.example/authorize?state=abc",
  redirectUri: "http://localhost:1455/auth/callback",
  expiresInSeconds: 600,
};

test("renders the open drawer with an active session form", () => {
  render(
    <OAuthLoginDrawer
      open
      provider="codex"
      session={session}
      pending={null}
      error={null}
      onClose={() => undefined}
      onRestart={() => undefined}
      onExchange={async () => undefined}
    />,
  );

  expect(screen.getByRole("dialog", { name: "Codex OAuth 认证" })).toBeInTheDocument();
  expect(screen.queryByText("Codex 授权会话")).not.toBeInTheDocument();
  expect(screen.queryByText("http://localhost:1455/auth/callback")).not.toBeInTheDocument();
  expect(screen.queryByText(/期望跳转/)).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "打开授权页" })).toHaveAttribute(
    "href",
    "https://auth.example/authorize?state=abc",
  );
  expect(screen.getByLabelText("回调 URL")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "激活账号" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "重新开始" })).toBeInTheDocument();
});

test("does not mount the drawer while closed", () => {
  render(
    <OAuthLoginDrawer
      open={false}
      provider="codex"
      session={null}
      pending={null}
      error={null}
      onClose={() => undefined}
      onRestart={() => undefined}
      onExchange={async () => undefined}
    />,
  );
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.queryByText("还没有 Codex 登录会话")).not.toBeInTheDocument();
});

test("submits the callback URL through onExchange", async () => {
  const onExchange = vi.fn<(callbackUrl: string) => Promise<void>>(async () => undefined);
  render(
    <OAuthLoginDrawer
      open
      provider="codex"
      session={session}
      pending={null}
      error={null}
      onClose={() => undefined}
      onRestart={() => undefined}
      onExchange={onExchange}
    />,
  );

  const input = screen.getByLabelText("回调 URL");
  fireEvent.change(input, {
    target: { value: "http://localhost:1455/auth/callback?code=abc&state=abc" },
  });
  fireEvent.submit(input.closest("form")!);

  expect(onExchange).toHaveBeenCalledTimes(1);
  expect(onExchange).toHaveBeenCalledWith(
    "http://localhost:1455/auth/callback?code=abc&state=abc",
  );
});

test("shows loading state while starting a session", () => {
  render(
    <OAuthLoginDrawer
      open
      provider="codex"
      session={null}
      pending="start"
      error={null}
      onClose={() => undefined}
      onRestart={() => undefined}
      onExchange={async () => undefined}
    />,
  );
  expect(screen.getByText("正在创建授权会话…")).toBeInTheDocument();
});
