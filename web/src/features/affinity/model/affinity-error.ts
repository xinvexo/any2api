export function getAffinityErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "会话运行态读取失败";
}
