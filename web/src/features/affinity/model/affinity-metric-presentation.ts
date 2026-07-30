import type { AffinityRuntime } from "../api/affinity-contracts";

export interface AffinityMetricPresentation {
  label: string;
  value: string;
  note: string;
}

export function describeAffinityMetrics(runtime: AffinityRuntime | undefined) {
  if (!runtime) {
    return {
      active: metric("活动显式会话", "—", "运行态暂不可用"),
      creating: metric("建立中显式会话", "—", "运行态暂不可用"),
    };
  }
  if (!runtime.affinityEnabled) {
    return {
      active: metric("活动显式会话", "已关闭", "可在“设置 → 路由策略”中启用"),
      creating: metric("建立中显式会话", "—", "显式会话粘性未启用"),
    };
  }
  return {
    active: metric(
      "活动显式会话",
      runtime.activeSessionCount.toLocaleString("zh-CN"),
      "TTL 内仍会命中的显式 Session，不含 Response ID 续接",
    ),
    creating: metric(
      "建立中显式会话",
      runtime.creatingSessionCount.toLocaleString("zh-CN"),
      "仅首次绑定提交前，通常很快归零",
    ),
  };
}

function metric(label: string, value: string, note: string): AffinityMetricPresentation {
  return { label, value, note };
}
