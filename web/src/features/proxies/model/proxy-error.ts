export function getProxyErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "出口代理配置操作失败";
}
