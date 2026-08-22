import type { InfiniteData } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

import type { SystemLog, SystemLogList } from "../api/system-log-contracts";
import {
  MAX_CACHED_SYSTEM_LOGS,
  countNewSystemLogs,
  flattenSystemLogPages,
  mergeLatestSystemBatches,
} from "./system-log-feed";

describe("system log feed merging", () => {
  it("deduplicates rows and caps the rendered summary cache at 3000", () => {
    const pages = Array.from({ length: 31 }, (_, page) =>
      batch(
        Array.from({ length: 100 }, (_, row) =>
          systemLog(`request-${page}-${row}`, 100_000 - page * 100 - row),
        ),
      ),
    );
    pages[1].items[0] = pages[0].items[0];

    const feed = flattenSystemLogPages(pages);
    expect(feed.items).toHaveLength(MAX_CACHED_SYSTEM_LOGS);
    expect(feed.items.filter((item) => item.requestId === "request-0-0")).toHaveLength(1);
  });

  it("drops the oldest page when a latest batch is merged into a full cache", () => {
    const current: InfiniteData<SystemLogList, string | null> = {
      pages: Array.from({ length: 30 }, (_, index) =>
        batch([systemLog(`old-${index}`, 10_000 - index)]),
      ),
      pageParams: [null, ...Array.from({ length: 29 }, (_, index) => `s5.${index}`)],
    };

    const merged = mergeLatestSystemBatches(
      current,
      data([batch([systemLog("latest", 20_000)])]),
    );
    const ids = merged.pages.flatMap((page) => page.items.map((item) => item.requestId));

    expect(merged.pages).toHaveLength(30);
    expect(ids[0]).toBe("latest");
    expect(ids).toContain("old-28");
    expect(ids).not.toContain("old-29");
  });

  it("keeps every recovered batch ahead of cached history", () => {
    const current = data([
      batch([systemLog("known", 10_000)]),
      batch([systemLog("older", 9_000)]),
    ]);
    const recovered = data(
      [
        batch([systemLog("new-1", 20_000)]),
        batch([systemLog("new-2", 19_000), systemLog("known", 10_000)]),
      ],
      [null, "s5.recovery"],
    );

    const merged = mergeLatestSystemBatches(current, recovered);

    expect(merged.pages.flatMap((page) => page.items.map((item) => item.requestId))).toEqual([
      "new-1",
      "new-2",
      "known",
      "older",
    ]);
  });

  it("does not report a reconnect snapshot as new when every id is known", () => {
    const current = data([batch([systemLog("known", 10_000)])]);
    const recovered = data([batch([systemLog("known", 10_000)])]);

    expect(countNewSystemLogs(current, recovered)).toBe(0);
  });
});

function data(
  pages: SystemLogList[],
  pageParams: Array<string | null> = pages.map((_, index) =>
    index === 0 ? null : `s5.${index}`,
  ),
): InfiniteData<SystemLogList, string | null> {
  return { pages, pageParams };
}

function batch(items: SystemLog[]): SystemLogList {
  return {
    items,
    nextCursor: null,
    hasMore: false,
    telemetry: {
      queuedRecords: 0,
      inFlightRecords: 0,
      droppedRecords: 0,
      persistedRecords: items.length,
    },
  };
}

function systemLog(requestId: string, startedAtMs: number): SystemLog {
  return {
    requestId,
    startedAtMs,
    configRevision: 1,
    clientIp: "127.0.0.1",
    method: "GET",
    path: "/v1/models",
    httpVersion: "HTTP/1.1",
    statusCode: 200,
    durationMs: 2,
    responseBytes: 128,
    outcome: "completed",
  };
}
