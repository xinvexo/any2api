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
) {
  if (!result || result.proxyId !== proxy.id) {
    return false;
  }
  return (
    result.configRevision === configRevision &&
    result.proxyConfigVersion === proxy.configVersion
  );
}

export function formatProxyTestDiagnostic(result: ProxyTestResult) {
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
  probe_target: "探测站点",
  proxy: "代理",
  unattributed: "未归因",
};
