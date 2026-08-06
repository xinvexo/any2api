import type {
  RequestAttempt,
  RequestAttemptFailureScope,
  RequestAttemptRetryDecision,
  RequestRoutingMode,
} from "../api/request-log-contracts";

const routingModeLabels: Record<RequestRoutingMode, string> = {
  balanced: "负载均衡",
  bound: "会话绑定",
};

const failureScopeLabels: Record<RequestAttemptFailureScope, string> = {
  unattributed: "未归因",
  authentication: "认证版本",
  credential: "凭据",
  credential_model: "凭据与模型",
  route_operation: "路由目标与操作",
  exact_candidate: "当前候选",
  egress_path: "Endpoint × 出口",
  proxy: "代理",
  endpoint: "Endpoint",
};

const retryDecisionLabels: Record<RequestAttemptRetryDecision, string> = {
  terminal: "终止",
  oauth_refresh: "刷新 OAuth",
  retry_same_path: "原路径重试",
  reselect: "重新选路",
};

export function attemptDiagnosticSummary(attempt: RequestAttempt) {
  const parts = [
    attempt.routingMode === null ? null : routingModeLabels[attempt.routingMode],
    attempt.failureScope === null
      ? null
      : `失败范围：${failureScopeLabels[attempt.failureScope]}`,
    attempt.retryDecision === null
      ? null
      : `决策：${retryDecisionLabels[attempt.retryDecision]}`,
  ].filter((part): part is string => part !== null);
  return parts.length === 0 ? null : parts.join(" · ");
}
