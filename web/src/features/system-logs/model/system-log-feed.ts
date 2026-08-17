import type { InfiniteData } from "@tanstack/react-query";

import type { SystemLog, SystemLogList } from "../api/system-log-contracts";

export const MAX_CACHED_SYSTEM_LOGS = 3_000;
export const MAX_CACHED_SYSTEM_BATCHES = 30;

export function flattenSystemLogPages(pages: readonly SystemLogList[]) {
  const unique = new Map<string, SystemLog>();
  for (const page of pages) {
    for (const item of page.items) {
      if (!unique.has(item.requestId)) {
        unique.set(item.requestId, item);
      }
    }
  }
  return {
    items: [...unique.values()].slice(0, MAX_CACHED_SYSTEM_LOGS),
    telemetry: pages[0]?.telemetry,
  };
}

export function mergeLatestSystemBatches(
  current: InfiniteData<SystemLogList, string | null> | undefined,
  latest: InfiniteData<SystemLogList, string | null>,
): InfiniteData<SystemLogList, string | null> {
  const pages: SystemLogList[] = [];
  const pageParams: Array<string | null> = [];
  const seen = new Set<string>();
  const candidates = [
    ...latest.pages.map((page, index) => ({ page, pageParam: latest.pageParams[index] })),
    ...(current?.pages ?? []).map((page, index) => ({
      page,
      pageParam: current?.pageParams[index],
    })),
  ];
  for (const { page, pageParam } of candidates) {
    if (pages.length >= MAX_CACHED_SYSTEM_BATCHES) {
      break;
    }
    const items = page.items.filter((item) => {
      if (seen.has(item.requestId)) {
        return false;
      }
      seen.add(item.requestId);
      return true;
    });
    if (items.length === 0 && pages.length > 0) {
      continue;
    }
    pages.push({ ...page, items });
    pageParams.push(pageParam ?? null);
  }
  return { pages, pageParams };
}

export function countNewSystemLogs(
  current: InfiniteData<SystemLogList, string | null>,
  latest: InfiniteData<SystemLogList, string | null>,
) {
  const known = systemLogIds(current);
  const incoming = new Set<string>();
  for (const page of latest.pages) {
    for (const item of page.items) {
      if (!known.has(item.requestId)) {
        incoming.add(item.requestId);
      }
    }
  }
  return incoming.size;
}

export function systemLogIds(data: InfiniteData<SystemLogList, string | null>) {
  return new Set(
    data.pages.flatMap((page) => page.items.map((item) => item.requestId)),
  );
}
