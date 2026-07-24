export const OAUTH_PAGE_SIZE_OPTIONS = [10, 20, 50] as const;
export type OAuthPageSize = (typeof OAUTH_PAGE_SIZE_OPTIONS)[number];

export function isOAuthPageSize(value: number): value is OAuthPageSize {
  return (OAUTH_PAGE_SIZE_OPTIONS as readonly number[]).includes(value);
}

export function paginateItems<T>(items: readonly T[], page: number, pageSize: number): T[] {
  const totalPages = Math.max(1, Math.ceil(items.length / pageSize));
  const safePage = Math.min(Math.max(1, page), totalPages);
  const start = (safePage - 1) * pageSize;
  return items.slice(start, start + pageSize);
}
