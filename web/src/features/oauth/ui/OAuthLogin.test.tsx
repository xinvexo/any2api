import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type { OAuthStartResult } from "../api/oauth-contracts";
import { OAuthLoginDrawer } from "./OAuthLogin";
import type { ProxyConfiguration } from "@/features/proxies";

afterEach(() => vi.restoreAllMocks());

const session: OAuthStartResult = {
  flow: "authorization_code",
  provider: "codex",
  sessionId: "session-1",
  authorizationUrl: "https://auth.example/authorize?state=abc",
  redirectUri: "http://localhost:1455/auth/callback",
  expiresInSeconds: 600,
};

const deviceSession: OAuthStartResult = {
  flow: "device_code",
  provider: "grok",
  sessionId: "grok-session",
  userCode: "ABCD-1234",
  verificationUri: "https://accounts.x.ai/oauth2/device",
  verificationUriComplete:
    "https://accounts.x.ai/oauth2/device?user_code=ABCD-1234",
  expiresInSeconds: 1800,
  pollIntervalSeconds: 5,
};

const proxyConfiguration: ProxyConfiguration = {
  configRevision: 1,
  globalProxyId: "proxy-1",
  items: [
    {
      id: "00000000-0000-0000-0000-000000000000",
      name: "DIRECT",
      kind: "direct",
      host: null,
      port: null,
      username: null,
      passwordConfigured: false,
      authenticationVersion: 0,
      enabled: true,
      builtIn: true,
      configVersion: 1,
    },
    {
      id: "proxy-1",
      name: "Office Proxy",
      kind: "http",
      host: "proxy.example.com",
      port: 8080,
      username: null,
      passwordConfigured: false,
      authenticationVersion: 0,
      enabled: true,
      builtIn: false,
      configVersion: 1,
    },
  ],
};

test("renders the open drawer with an active session form", () => {
  render(
    <OAuthLoginDrawer
      open
      provider="codex"
      proxySelection={{ mode: "global" }}
      proxyConfiguration={proxyConfiguration}
      session={session}
      pending={null}
      error={null}
      onClose={() => undefined}
      onProxySelectionChange={() => undefined}
      onStart={() => undefined}
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
      proxySelection={{ mode: "global" }}
      proxyConfiguration={proxyConfiguration}
      session={null}
      pending={null}
      error={null}
      onClose={() => undefined}
      onProxySelectionChange={() => undefined}
      onStart={() => undefined}
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
      proxySelection={{ mode: "global" }}
      proxyConfiguration={proxyConfiguration}
      session={session}
      pending={null}
      error={null}
      onClose={() => undefined}
      onProxySelectionChange={() => undefined}
      onStart={() => undefined}
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
      proxySelection={{ mode: "global" }}
      proxyConfiguration={proxyConfiguration}
      session={null}
      pending="start"
      error={null}
      onClose={() => undefined}
      onProxySelectionChange={() => undefined}
      onStart={() => undefined}
      onExchange={async () => undefined}
    />,
  );
  expect(screen.getByText("正在创建授权会话…")).toBeInTheDocument();
});

test("renders Grok device authorization without a callback form", () => {
  render(
    <OAuthLoginDrawer
      open
      provider="grok"
      proxySelection={{ mode: "global" }}
      proxyConfiguration={proxyConfiguration}
      session={deviceSession}
      pending="poll"
      error={null}
      onClose={() => undefined}
      onProxySelectionChange={() => undefined}
      onStart={() => undefined}
      onExchange={async () => undefined}
    />,
  );

  expect(screen.getByLabelText("设备授权码")).toHaveTextContent("ABCD-1234");
  expect(screen.getByRole("link", { name: "打开验证页" })).toHaveAttribute(
    "href",
    "https://accounts.x.ai/oauth2/device?user_code=ABCD-1234",
  );
  expect(screen.getByText("正在确认授权状态…")).toBeInTheDocument();
  expect(screen.queryByLabelText("回调 URL")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "激活账号" })).not.toBeInTheDocument();
});

test("selects an OAuth proxy before starting login", () => {
  const onProxySelectionChange = vi.fn();
  const onStart = vi.fn();
  render(
    <OAuthLoginDrawer
      open
      provider="codex"
      proxySelection={{ mode: "global" }}
      proxyConfiguration={proxyConfiguration}
      session={null}
      pending={null}
      error={null}
      onClose={() => undefined}
      onProxySelectionChange={onProxySelectionChange}
      onStart={onStart}
      onExchange={async () => undefined}
    />,
  );

  expect(screen.getByRole("combobox", { name: "OAuth 出口" })).toHaveTextContent(
    "跟随 OAuth 全局出口（Office Proxy，HTTP）",
  );
  fireEvent.click(screen.getByRole("combobox", { name: "OAuth 出口" }));
  fireEvent.click(screen.getByRole("option", { name: "DIRECT，本机直连" }));
  expect(onProxySelectionChange).toHaveBeenCalledWith({
    mode: "profile",
    proxyProfileId: "00000000-0000-0000-0000-000000000000",
  });

  fireEvent.click(screen.getByRole("button", { name: "开始认证" }));
  expect(onStart).toHaveBeenCalledTimes(1);
});
