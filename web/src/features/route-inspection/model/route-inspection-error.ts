export function getRouteInspectionErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "路由检查读取失败";
}
