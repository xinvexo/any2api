export function getAffinityErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "会话粘性操作失败";
}
