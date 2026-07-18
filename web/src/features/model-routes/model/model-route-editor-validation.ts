import type { ProviderEndpoint } from "@/features/providers";

import type {
  ModelRouteEditorDraft,
  ModelRouteEditorErrors,
  RouteTargetEditorErrors,
} from "./model-route-editor-types";

export function validateModelRouteDraft(
  draft: ModelRouteEditorDraft,
  endpoints: ProviderEndpoint[],
): ModelRouteEditorErrors {
  const errors: ModelRouteEditorErrors = { targetByClientId: {} };
  errors.publicModel = validateModelName(draft.publicModel, "请输入公开模型名");
  if (draft.targets.length === 0) {
    errors.targets = "至少需要一个 Route Target";
    return errors;
  }
  if (draft.enabled && !draft.targets.some((target) => target.enabled)) {
    errors.targets = "启用的模型路由至少需要一个启用的 Target";
  }

  const identities = new Set<string>();
  for (const target of draft.targets) {
    const targetErrors: RouteTargetEditorErrors = {};
    const endpoint = endpoints.find((item) => item.id === target.providerEndpointId);
    if (!endpoint) {
      targetErrors.providerEndpointId = "请选择 Provider Endpoint";
    } else if (endpoint.protocolDialect !== draft.ingressProtocol) {
      targetErrors.providerEndpointId = "Endpoint 协议与入口协议不一致";
    }
    targetErrors.upstreamModel = validateModelName(target.upstreamModel, "请输入上游模型名");
    const tier = Number(target.fallbackTier);
    if (
      target.fallbackTier.trim().length === 0 ||
      !Number.isSafeInteger(tier) ||
      tier < 0 ||
      tier > 65_535
    ) {
      targetErrors.fallbackTier = "Tier 必须是 0 到 65535 的整数";
    }

    const identity = `${target.providerEndpointId}\u0000${target.upstreamModel}`;
    if (!targetErrors.providerEndpointId && !targetErrors.upstreamModel) {
      if (identities.has(identity)) {
        targetErrors.upstreamModel = "同一 Endpoint 不能重复配置相同上游模型";
      }
      identities.add(identity);
    }
    if (Object.values(targetErrors).some(Boolean)) {
      errors.targetByClientId[target.clientId] = targetErrors;
    }
  }
  return errors;
}

export function modelRouteEditorHasErrors(errors: ModelRouteEditorErrors) {
  return (
    Boolean(errors.publicModel || errors.targets) ||
    Object.values(errors.targetByClientId).some((target) =>
      Object.values(target).some(Boolean),
    )
  );
}

function validateModelName(value: string, emptyMessage: string): string | undefined {
  if (value.length === 0) {
    return emptyMessage;
  }
  if (value.trim() !== value) {
    return "模型名首尾不能包含空白";
  }
  if ([...value].length > 255) {
    return "模型名不能超过 255 个字符";
  }
  if ([...value].some((character) => /\p{Cc}/u.test(character))) {
    return "模型名不能包含控制字符";
  }
  return undefined;
}
