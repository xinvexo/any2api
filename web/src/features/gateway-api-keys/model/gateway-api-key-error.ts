import { ApiError } from "@/shared/api/http-client";

export function getGatewayApiKeyErrorMessage(error: unknown) {
  if (error instanceof ApiError) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "网关密钥请求失败，请稍后重试。";
}
