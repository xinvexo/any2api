import type { ProviderEndpoint } from "@/features/providers";
import type { ProxyProfile, ProxyTestResult } from "../api/proxy-contracts";

export function isCurrentTestResult(
  result: ProxyTestResult | undefined,
  proxy: ProxyProfile,
  configRevision: number,
  endpoints: ProviderEndpoint[],
  selectedEndpointId: string,
) {
  if (!result || result.providerEndpointId !== selectedEndpointId) {
    return false;
  }
  const endpoint = endpoints.find((candidate) => candidate.id === result.providerEndpointId);
  return (
    result.configRevision === configRevision &&
    result.proxyConfigVersion === proxy.configVersion &&
    endpoint?.configVersion === result.providerEndpointConfigVersion
  );
}
