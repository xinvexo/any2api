export function getOverviewUsageErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "调用统计读取失败";
}
