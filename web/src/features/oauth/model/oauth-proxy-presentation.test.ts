import { expect, test } from "vitest";

import { describeOAuthProxySelection } from "./oauth-proxy-presentation";
import type { ProxyConfiguration } from "@/features/proxies";

test("marks a disabled explicit OAuth proxy as unavailable for routing", () => {
  const configuration = {
    configRevision: 1,
    globalProxyId: "direct",
    items: [
      profile("direct", "DIRECT", "direct", true),
      profile("proxy-1", "Office", "http", false),
    ],
  } satisfies ProxyConfiguration;

  expect(
    describeOAuthProxySelection(
      { mode: "profile", proxyProfileId: "proxy-1" },
      configuration,
    ),
  ).toBe("指定 · Office，HTTP · 已停用");
  expect(
    describeOAuthProxySelection(
      { mode: "profile", proxyProfileId: "deleted" },
      configuration,
    ),
  ).toBe("指定代理已删除");
});

function profile(
  id: string,
  name: string,
  kind: "direct" | "http",
  enabled: boolean,
) {
  return {
    id,
    name,
    kind,
    host: kind === "direct" ? null : "proxy.example.com",
    port: kind === "direct" ? null : 8080,
    username: null,
    passwordConfigured: false,
    authenticationVersion: 0,
    enabled,
    builtIn: kind === "direct",
    configVersion: 1,
  };
}
