import type { InfiniteData } from "@tanstack/react-query";

import type {
  ActiveRequestLog,
  RequestLog,
  RequestLogList,
} from "../api/request-log-contracts";

export const MAX_CACHED_REQUEST_LOGS = 3_000;
export const MAX_CACHED_REQUEST_BATCHES = 29;

export type RequestLogFeedItem = ActiveRequestLog | RequestLog;

export function isActiveRequestLog(
  item: RequestLogFeedItem,
): item is ActiveRequestLog {
  return "state" in item && item.state === "processing";
}

export function flattenRequestLogPages(pages: readonly RequestLogList[]) {
  const completed = new Map<string, RequestLog>();
  for (const page of pages) {
    for (const item of page.items) {
      if (!completed.has(item.requestId)) {
        completed.set(item.requestId, item);
      }
    }
  }
  const completedIds = new Set(completed.keys());
  const active = (pages[0]?.activeItems ?? []).filter(
    (item, index, items) =>
      !completedIds.has(item.requestId) &&
      items.findIndex((candidate) => candidate.requestId === item.requestId) === index,
  );
  const items = [...active, ...completed.values()].sort(compareNewestFirst);
  return {
    items: items.slice(0, MAX_CACHED_REQUEST_LOGS),
    telemetry: pages[0]?.telemetry,
    filterOptions: pages[0]?.filterOptions,
    activeTotal: pages[0]?.activeTotal ?? 0,
  };
}

export function mergeLatestRequestBatches(
  current: InfiniteData<RequestLogList, string | null> | undefined,
  latest: InfiniteData<RequestLogList, string | null>,
): InfiniteData<RequestLogList, string | null> {
  const pages: RequestLogList[] = [];
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
    if (pages.length >= MAX_CACHED_REQUEST_BATCHES) {
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
    pages.push(
      pages.length === 0
        ? { ...page, items }
        : { ...page, activeItems: [], activeTotal: 0, items },
    );
    pageParams.push(pageParam ?? null);
  }
  return { pages, pageParams };
}

export function countNewRequestLogs(
  current: InfiniteData<RequestLogList, string | null>,
  latest: InfiniteData<RequestLogList, string | null>,
) {
  const known = requestLogIds(current);
  const active = new Set(
    current.pages.flatMap((page) => page.activeItems.map((item) => item.requestId)),
  );
  const completed = completedRequestLogIds(current);
  const changed = new Set<string>();
  for (const page of latest.pages) {
    for (const item of page.activeItems) {
      if (!known.has(item.requestId)) {
        changed.add(item.requestId);
      }
    }
    for (const item of page.items) {
      if (
        !known.has(item.requestId) ||
        (active.has(item.requestId) && !completed.has(item.requestId))
      ) {
        changed.add(item.requestId);
      }
    }
  }
  return changed.size;
}

export function completedRequestLogIds(
  data: InfiniteData<RequestLogList, string | null>,
) {
  return new Set(
    data.pages.flatMap((page) => page.items.map((item) => item.requestId)),
  );
}

function requestLogIds(data: InfiniteData<RequestLogList, string | null>) {
  return new Set(
    data.pages.flatMap((page) => [
      ...page.activeItems.map((item) => item.requestId),
      ...page.items.map((item) => item.requestId),
    ]),
  );
}

function compareNewestFirst(left: RequestLogFeedItem, right: RequestLogFeedItem) {
  return right.startedAtMs - left.startedAtMs || right.requestId.localeCompare(left.requestId);
}
