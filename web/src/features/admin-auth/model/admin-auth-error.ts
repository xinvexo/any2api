import { ApiError } from "@/shared/api/http-client";

export function getAdminAuthErrorMessage(error: unknown) {
  if (error instanceof ApiError) {
    if (error.code === "admin_remote_disabled") {
      return "当前实例没有启用远程管理。请从本机访问并在设置中开启。";
    }
    if (error.code === "admin_invalid_credentials") {
      return "管理员密码不正确。";
    }
    if (error.code === "admin_login_rate_limited") {
      return "登录失败次数过多，请稍后重试。";
    }
    if (error.code === "admin_invalid_password") {
      return "密码长度需要在 12 到 1024 字节之间。";
    }
    if (error.code === "admin_invalid_setup_token") {
      return "Setup Token 不正确或已经失效。";
    }
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "无法连接管理服务。";
}
