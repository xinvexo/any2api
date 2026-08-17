import type { InfiniteData } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

import type {
  ActiveRequestLog,
  RequestLog,
  RequestLogList,
} from "../api/request-log-contracts";
import {
  MAX_CACHED_REQUEST_LOGS,
  countNewRequestLogs,
  flattenRequestLogPages,
  isActiveRequestLog,
  mergeLatestRequestBatches,
} from "./request-log-feed";

describe("request log feed merging", () => {
  it("deduplicates an active request once its completed row appears", () => {
    const feed = flattenRequestLogPages([
      batch(
        [completed("completed-request", 200)],
        [active("completed-request", 300), active("live-request", 400)],
      ),
    ]);

    expect(feed.items.map((item) => item.requestId)).toEqual([
      "live-request",
      "completed-request",
    ]);
    expect(feed.items.filter(isActiveRequestLog)).toHaveLength(1);
  });

  it("caps the rendered summary cache at 3000 rows", () => {
    const pages = Array.from({ length: 31 }, (_, page) =>
      batch(
        Array.from({ length: 100 }, (_, row) =>
          completed(`request-${page}-${row}`, 100_000 - page * 100 - row),
        ),
      ),
    );

    expect(flattenRequestLogPages(pages).items).toHaveLength(MAX_CACHED_REQUEST_LOGS);
  });

  it("drops the oldest page when a latest batch is merged into a full cache", () => {
    const current: InfiniteData<RequestLogList, string | null> = {
      pages: Array.from({ length: 29 }, (_, index) =>
        batch([completed(`old-${index}`, 10_000 - index)]),
      ),
      pageParams: [null, ...Array.from({ length: 28 }, (_, index) => `r4.${index}`)],
    };

    const merged = mergeLatestRequestBatches(
      current,
      data([batch([completed("latest", 20_000)])]),
    );
    const ids = merged.pages.flatMap((page) => page.items.map((item) => item.requestId));

    expect(merged.pages).toHaveLength(29);
    expect(ids[0]).toBe("latest");
    expect(ids).toContain("old-27");
    expect(ids).not.toContain("old-28");
  });

  it("keeps every recovered batch ahead of cached history", () => {
    const current = data([
      batch([completed("known", 10_000)]),
      batch([completed("older", 9_000)]),
    ]);
    const recovered = data(
      [
        batch([completed("new-1", 20_000)]),
        batch([completed("new-2", 19_000), completed("known", 10_000)]),
      ],
      [null, "r4.recovery"],
    );

    const merged = mergeLatestRequestBatches(current, recovered);

    expect(merged.pages.flatMap((page) => page.items.map((item) => item.requestId))).toEqual([
      "new-1",
      "new-2",
      "known",
      "older",
    ]);
  });

  it("counts an active request becoming persisted as one pending update", () => {
    const current = data([batch([], [active("transition", 20_000)])]);
    const recovered = data([batch([completed("transition", 20_000)])]);

    expect(countNewRequestLogs(current, recovered)).toBe(1);
  });
});

function data(
  pages: RequestLogList[],
  pageParams: Array<string | null> = pages.map((_, index) =>
    index === 0 ? null : `r4.${index}`,
  ),
): InfiniteData<RequestLogList, string | null> {
  return { pages, pageParams };
}

function batch(
  items: RequestLog[],
  activeItems: ActiveRequestLog[] = [],
): RequestLogList {
  return {
    activeItems,
    activeTotal: activeItems.length,
    items,
    nextCursor: null,
    hasMore: false,
    telemetry: {
      queuedRecords: 0,
      inFlightRecords: 0,
      droppedRecords: 0,
      persistedRecords: items.length,
    },
    filterOptions: { publicModels: [], gatewayApiKeys: [] },
  };
}

function completed(requestId: string, startedAtMs: number): RequestLog {
  return {
    requestId,
    startedAtMs,
    clientIp: "127.0.0.1",
    configRevision: 1,
    gatewayApiKeyId: null,
    ingressProtocol: "openai_responses",
    operation: "responses",
    publicModel: "codex-test",
    thinkingLevel: null,
    providerEndpointId: null,
    providerEndpointName: null,
    credentialId: null,
    credentialLabel: null,
    oauthAccountId: null,
    oauthAccountLabel: null,
    proxyProfileId: null,
    proxyProfileLabel: null,
    statusCode: 200,
    outcome: "success",
    errorMessage: null,
    attemptCount: 1,
    latencyMs: 10,
    firstTokenMs: 2,
    inputTokens: 1,
    outputTokens: 1,
    cacheReadTokens: 0,
    cacheCreationTokens: 0,
    isStream: true,
  };
}

function active(requestId: string, startedAtMs: number): ActiveRequestLog {
  return {
    state: "processing",
    requestId,
    startedAtMs,
    clientIp: "127.0.0.1",
    configRevision: 1,
    gatewayApiKeyId: "gateway-key",
    ingressProtocol: "openai_responses",
    operation: "responses",
    publicModel: "codex-test",
    thinkingLevel: null,
    providerEndpointId: null,
    providerEndpointName: null,
    credentialId: null,
    credentialLabel: null,
    oauthAccountId: null,
    oauthAccountLabel: null,
    proxyProfileId: null,
    proxyProfileLabel: null,
    attemptCount: 0,
    isStream: true,
  };
}
