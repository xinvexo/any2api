import type { OAuthProxySelection } from "../api/oauth-contracts";
import type { ProxyConfiguration, ProxyProfile } from "@/features/proxies";

export function describeOAuthProxySelection(
  selection: OAuthProxySelection,
  configuration: ProxyConfiguration,
) {
  if (selection.mode === "global") {
    const globalProxy = configuration.items.find(
      (profile) => profile.id === configuration.globalProxyId,
    );
    return `跟随 OAuth 全局出口 · ${describeProxyProfile(globalProxy)}`;
  }
  const profile = configuration.items.find(
    (item) => item.id === selection.proxyProfileId,
  );
  return profile ? `指定 · ${describeProxyProfile(profile)}` : "指定代理已删除";
}

export function describeProxyProfile(profile: ProxyProfile | undefined) {
  if (!profile) {
    return "不可用";
  }
  if (profile.kind === "direct") {
    return "DIRECT，本机直连";
  }
  return `${profile.name}，${profile.kind.toUpperCase()}`;
}
