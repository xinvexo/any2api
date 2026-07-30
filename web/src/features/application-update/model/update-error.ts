import { ApiError } from "@/shared/api/http-client";

const messages: Record<string, string> = {
  update_unavailable: "当前服务没有启用版本更新能力。",
  update_unsupported: "当前运行环境不支持自动更新。",
  update_not_available: "当前已经是最新版本。",
  update_in_progress: "已有更新正在进行，请稍候。",
  update_check_failed: "无法验证 GitHub 上的最新正式版本。",
  update_download_failed: "官方版本下载失败，请检查网络后重试。",
  update_verification_failed: "下载文件未通过校验，当前版本未被替换。",
  update_install_failed: "新版本已验证，但无法替换当前程序。请检查安装目录权限。",
};

export function getUpdateFailureMessage(code: string) {
  return messages[code] ?? "版本更新操作失败";
}

export function getUpdateErrorMessage(error: unknown) {
  if (error instanceof ApiError) {
    return messages[error.code] ?? error.message;
  }
  return error instanceof Error ? error.message : "版本更新操作失败";
}
