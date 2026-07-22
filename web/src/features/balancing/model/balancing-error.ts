export function getBalancingErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "负载均衡运行态读取失败";
}
