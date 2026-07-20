import { ApiError } from "@/shared/api/http-client";

export function getRequestLogErrorMessage(error: unknown) {
  if (error instanceof ApiError) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "请求日志暂时不可用";
}
