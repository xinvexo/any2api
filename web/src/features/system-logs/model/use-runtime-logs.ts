import { useInfiniteQuery, useQueryClient, type InfiniteData } from "@tanstack/react-query";

import { getRuntimeLogs } from "../api/runtime-log-api";
import type { RuntimeLogPage } from "@/shared/api/generated/RuntimeLogPage";

export function useRuntimeLogs(followingLatest: boolean) {
  const client = useQueryClient();
  const queryKey = ["runtime-logs"] as const;
  const query = useInfiniteQuery({
    queryKey,
    queryFn: ({ pageParam, signal }) => getRuntimeLogs(pageParam, signal),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
    maxPages: 10,
    staleTime: 5_000,
    gcTime: 60_000,
    refetchOnWindowFocus: false,
    refetchInterval: (query) => followingLatest && (query.state.data?.pages.length ?? 1) === 1 ? 10_000 : false,
  });

  async function refresh() {
    await client.cancelQueries({ queryKey, exact: true });
    client.setQueryData<InfiniteData<RuntimeLogPage, string | null>>(queryKey, (current) => current
      ? { pages: current.pages.slice(0, 1), pageParams: [null] }
      : current);
    return query.refetch();
  }

  return {
    ...query,
    items: query.data?.pages.flatMap((page) => page.items) ?? [],
    automatic: followingLatest && (query.data?.pages.length ?? 1) === 1,
    refresh,
  };
}
