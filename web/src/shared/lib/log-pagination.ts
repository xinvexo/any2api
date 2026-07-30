export const LOG_PAGE_SIZE_OPTIONS = [10, 20, 50] as const;
export type LogPageSize = (typeof LOG_PAGE_SIZE_OPTIONS)[number];

export function isLogPageSize(value: number): value is LogPageSize {
  return (LOG_PAGE_SIZE_OPTIONS as readonly number[]).includes(value);
}

export function logPageCount(total: number, pageSize: number) {
  return Math.max(1, Math.ceil(total / pageSize));
}
