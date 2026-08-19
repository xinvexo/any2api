import { ApiError } from "@/shared/api/http-client";

const messages: Record<string, string> = {
  update_in_progress: "版本更新正在进行，暂时无法重启。",
  restart_update_in_progress: "版本更新正在进行，暂时无法重启。",
  restart_unsupported: "当前运行环境不支持手动重启。",
  restart_unavailable: "重启功能暂时不可用，请稍后重试。",
};

export function getRestartErrorMessage(error: unknown) {
  if (error instanceof ApiError) {
    if (messages[error.code]) {
      return messages[error.code];
    }
    if (error.status === 409) {
      return "当前无法手动重启：版本更新正在进行，或运行环境不支持此操作。";
    }
    if (error.status === 503) {
      return "重启功能暂时不可用，请稍后重试。";
    }
    return error.message;
  }
  return "无法发起重启，请稍后重试。";
}
