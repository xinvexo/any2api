export function getProxyErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "代理配置操作失败";
}
