export function getProviderErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "Provider 配置操作失败";
}
