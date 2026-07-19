export function getSettingsErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "设置操作失败";
}
