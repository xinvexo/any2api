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
