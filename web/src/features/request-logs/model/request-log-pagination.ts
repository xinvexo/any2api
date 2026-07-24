export const REQUEST_LOG_PAGE_SIZE_OPTIONS = [10, 20, 50] as const;
export type RequestLogPageSize = (typeof REQUEST_LOG_PAGE_SIZE_OPTIONS)[number];

export function isRequestLogPageSize(value: number): value is RequestLogPageSize {
  return (REQUEST_LOG_PAGE_SIZE_OPTIONS as readonly number[]).includes(value);
}

export function paginateItems<T>(items: readonly T[], page: number, pageSize: number): T[] {
  const totalPages = Math.max(1, Math.ceil(items.length / pageSize));
  const safePage = Math.min(Math.max(1, page), totalPages);
  const start = (safePage - 1) * pageSize;
  return items.slice(start, start + pageSize);
}
