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
    return `跟随全局（${describeProxyProfile(globalProxy)}）`;
  }
  const profile = configuration.items.find(
    (item) => item.id === selection.proxyProfileId,
  );
  if (!profile) {
    return "指定代理已删除";
  }
  return profile.enabled
    ? `指定 · ${describeProxyProfile(profile)}`
    : `指定 · ${describeProxyProfile(profile)} · 已停用`;
}

export function describeProxyProfile(
  profile: Pick<ProxyProfile, "kind" | "name"> | undefined,
) {
  if (!profile) {
    return "不可用";
  }
  if (profile.kind === "direct") {
    return "DIRECT 本机直连";
  }
  return `${profile.kind.toUpperCase()} ${profile.name}`;
}
