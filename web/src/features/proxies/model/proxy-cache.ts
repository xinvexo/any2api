import type { ProxyConfiguration } from "../api/proxy-contracts";

export function selectNewestProxyConfiguration(
  current: ProxyConfiguration | undefined,
  incoming: ProxyConfiguration,
) {
  if (!current || incoming.configRevision >= current.configRevision) {
    return incoming;
  }
  return current;
}
