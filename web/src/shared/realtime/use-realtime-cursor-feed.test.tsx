import { QueryClient, QueryClientProvider, type InfiniteData } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { expect, test, vi } from "vitest";

import { useRealtimeCursorFeed } from "./use-realtime-cursor-feed";

test("coalesces change events and applies pending rows when following resumes", async () => {
  let latest = page(["known"]);
  const fetchPage = vi.fn(async () => latest);
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const rendered = renderHook(
    ({ followingLatest }) => useRealtimeCursorFeed<Item, Page, Feed>({
      queryKey: QUERY_KEY,
      scope: "all",
      followingLatest,
      fetchPage,
      knownIds,
      itemId: (item) => item.id,
      mergeLatest: (_current, incoming) => incoming,
      countNew,
      flatten: (pages) => ({ items: pages.flatMap((batch) => batch.items) }),
      maxCachedPages: 3,
      maxCollectedItems: 10,
      syncErrorMessage: "sync failed",
    }),
    { wrapper, initialProps: { followingLatest: false } },
  );

  await waitFor(() => expect(rendered.result.current.items).toEqual([{ id: "known" }]));
  await waitFor(() => expect(fetchPage).toHaveBeenCalledTimes(2));
  fetchPage.mockClear();
  latest = page(["new", "known"]);

  act(() => {
    rendered.result.current.scheduleSync();
    rendered.result.current.scheduleSync();
    rendered.result.current.scheduleSync();
  });
  await waitFor(() => expect(fetchPage).toHaveBeenCalledTimes(1));
  expect(rendered.result.current.items).toEqual([{ id: "known" }]);

  rendered.rerender({ followingLatest: true });
  await waitFor(() => expect(rendered.result.current.items).toEqual([
    { id: "new" },
    { id: "known" },
  ]));
});

test("aborts an old scope catch-up before starting the new scope", async () => {
  let oldScopeCalls = 0;
  let oldSyncSignal: AbortSignal | undefined;
  const fetchPage = vi.fn(
    (requestedScope: string, _cursor: string | null, signal?: AbortSignal) => {
      if (requestedScope === "old") {
        oldScopeCalls += 1;
        if (oldScopeCalls === 1) {
          return Promise.resolve(page(["known-old"]));
        }
        oldSyncSignal = signal;
        return pendingUntilAbort(signal);
      }
      return Promise.resolve(page(["known-new"]));
    },
  );
  const client = queryClient();
  const rendered = renderHook(
    ({ scope }) => useRealtimeCursorFeed<Item, Page, Feed>({
      queryKey: ["test-realtime-cursor-feed", scope],
      scope,
      followingLatest: true,
      fetchPage: (cursor, signal) => fetchPage(scope, cursor, signal),
      knownIds,
      itemId: (item) => item.id,
      mergeLatest: (_current, incoming) => incoming,
      countNew,
      flatten: flattenFeed,
      maxCachedPages: 3,
      maxCollectedItems: 10,
      syncErrorMessage: "sync failed",
    }),
    { wrapper: wrapper(client), initialProps: { scope: "old" } },
  );

  await waitFor(() => expect(oldSyncSignal).toBeDefined());
  rendered.rerender({ scope: "new" });

  await waitFor(() => expect(oldSyncSignal?.aborted).toBe(true));
  await waitFor(() => expect(rendered.result.current.items).toEqual([{ id: "known-new" }]));
  expect(rendered.result.current.isError).toBe(false);
});

test("manual refresh replaces an in-flight cursor catch-up", async () => {
  let callCount = 0;
  let syncSignal: AbortSignal | undefined;
  const fetchPage = vi.fn((_cursor: string | null, signal?: AbortSignal) => {
    callCount += 1;
    if (callCount === 1) {
      return Promise.resolve(page(["known"]));
    }
    if (callCount === 2) {
      syncSignal = signal;
      return pendingUntilAbort(signal);
    }
    return Promise.resolve(page(["fresh"]));
  });
  const client = queryClient();
  const rendered = renderHook(
    () => useRealtimeCursorFeed<Item, Page, Feed>({
      queryKey: QUERY_KEY,
      scope: "all",
      followingLatest: true,
      fetchPage,
      knownIds,
      itemId: (item) => item.id,
      mergeLatest: (_current, incoming) => incoming,
      countNew,
      flatten: flattenFeed,
      maxCachedPages: 3,
      maxCollectedItems: 10,
      syncErrorMessage: "sync failed",
    }),
    { wrapper: wrapper(client) },
  );

  await waitFor(() => expect(syncSignal).toBeDefined());
  let refreshed = false;
  await act(async () => {
    refreshed = await rendered.result.current.refreshLatest();
  });

  expect(refreshed).toBe(true);
  expect(syncSignal?.aborted).toBe(true);
  expect(rendered.result.current.items).toEqual([{ id: "fresh" }]);
  expect(rendered.result.current.isError).toBe(false);
});

test("aborts an in-flight cursor catch-up when the feed unmounts", async () => {
  let callCount = 0;
  let syncSignal: AbortSignal | undefined;
  const fetchPage = vi.fn((_cursor: string | null, signal?: AbortSignal) => {
    callCount += 1;
    if (callCount === 1) {
      return Promise.resolve(page(["known"]));
    }
    syncSignal = signal;
    return pendingUntilAbort(signal);
  });
  const client = queryClient();
  const rendered = renderHook(
    () => useRealtimeCursorFeed<Item, Page, Feed>({
      queryKey: QUERY_KEY,
      scope: "all",
      followingLatest: true,
      fetchPage,
      knownIds,
      itemId: (item) => item.id,
      mergeLatest: (_current, incoming) => incoming,
      countNew,
      flatten: flattenFeed,
      maxCachedPages: 3,
      maxCollectedItems: 10,
      syncErrorMessage: "sync failed",
    }),
    { wrapper: wrapper(client) },
  );

  await waitFor(() => expect(syncSignal).toBeDefined());
  rendered.unmount();
  expect(syncSignal?.aborted).toBe(true);
});

interface Item {
  id: string;
}

interface Page {
  items: Item[];
  nextCursor: string | null;
}

interface Feed {
  items: Item[];
}

const QUERY_KEY = ["test-realtime-cursor-feed"] as const;

function page(ids: string[]): Page {
  return { items: ids.map((id) => ({ id })), nextCursor: null };
}

function queryClient() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
}

function wrapper(client: QueryClient) {
  return function QueryWrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

function flattenFeed(pages: readonly Page[]): Feed {
  return { items: pages.flatMap((batch) => batch.items) };
}

function pendingUntilAbort(signal?: AbortSignal): Promise<Page> {
  return new Promise((_resolve, reject) => {
    const abort = () => reject(new DOMException("aborted", "AbortError"));
    if (signal?.aborted) {
      abort();
      return;
    }
    signal?.addEventListener("abort", abort, { once: true });
  });
}

function knownIds(data: InfiniteData<Page, string | null>) {
  return new Set(data.pages.flatMap((batch) => batch.items.map((item) => item.id)));
}

function countNew(
  current: InfiniteData<Page, string | null>,
  incoming: InfiniteData<Page, string | null>,
) {
  const known = knownIds(current);
  return incoming.pages.flatMap((batch) => batch.items)
    .filter((item) => !known.has(item.id)).length;
}
