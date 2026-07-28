import type { ProviderEndpoint } from "@/features/providers";
import type {
  ProxyProfile,
  ProxyTestFailureScope,
  ProxyTestFailureStage,
  ProxyTestResult,
} from "../api/proxy-contracts";

export function isCurrentTestResult(
  result: ProxyTestResult | undefined,
  proxy: ProxyProfile,
  configRevision: number,
  endpoints: ProviderEndpoint[],
  selectedEndpointId: string,
) {
  if (
    !result ||
    result.proxyId !== proxy.id ||
    result.providerEndpointId !== selectedEndpointId
  ) {
    return false;
  }
  const endpoint = endpoints.find((candidate) => candidate.id === result.providerEndpointId);
  return (
    result.configRevision === configRevision &&
    result.proxyConfigVersion === proxy.configVersion &&
    endpoint?.configVersion === result.providerEndpointConfigVersion
  );
}

export function formatProxyTestResult(result: ProxyTestResult) {
  if (result.reachable) {
    return `可达 · HTTP ${result.statusCode} · ${result.latencyMs} ms`;
  }
  return `失败 · ${stageLabels[result.errorStage!]} · ${scopeLabels[result.failureScope!]} · ${result.latencyMs} ms`;
}

const stageLabels: Record<ProxyTestFailureStage, string> = {
  dns: "DNS",
  tcp: "TCP",
  proxy_handshake: "代理握手",
  tls: "TLS",
  write_request: "写入请求",
  await_headers: "等待响应头",
  read_body: "读取响应体",
};

const scopeLabels: Record<ProxyTestFailureScope, string> = {
  endpoint: "Endpoint",
  proxy: "代理",
  unattributed: "未归因",
};
