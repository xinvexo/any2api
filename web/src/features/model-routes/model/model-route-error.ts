import { ApiError } from "@/shared/api/http-client";

export function getModelRouteErrorMessage(error: unknown) {
  if (error instanceof ApiError) {
    return messages[error.code] ?? "模型路由操作失败";
  }
  return error instanceof Error ? error.message : "模型路由操作失败";
}

const messages: Record<string, string> = {
  revision_conflict: "配置已发生变化，请刷新后重试。",
  model_route_not_found: "模型路由不存在或已被删除。",
  model_route_version_conflict: "此模型路由已被修改，请重新打开后再保存。",
  model_route_name_conflict: "同一入口协议不能使用重复的公开模型名。",
  route_target_identity_conflict: "已有 Target 的 Endpoint 或上游模型不能原地修改。",
  provider_endpoint_not_found: "所选 Provider Endpoint 不存在。",
  invalid_model_route: "模型路由配置无效。",
};
