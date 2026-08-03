import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { expect, test, vi } from "vitest";

import {
  selectNewestConfiguration,
  useConfigurationMutationLifecycle,
} from "./use-configuration-mutation-lifecycle";

interface Configuration {
  configRevision: number;
  value: string;
}

test("selects equal or newer revisions without moving backwards", () => {
  const current = configuration(4, "current");
  const equal = configuration(4, "equal response");

  expect(selectNewestConfiguration(undefined, current)).toBe(current);
  expect(selectNewestConfiguration(current, configuration(3, "stale"))).toBe(current);
  expect(selectNewestConfiguration(current, equal)).toBe(equal);
  expect(selectNewestConfiguration(current, configuration(5, "new"))).toEqual(
    configuration(5, "new"),
  );
});

test("publishes one cache, invalidates the declared scope and refetches active failures", async () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const invalidate = vi.spyOn(queryClient, "invalidateQueries").mockResolvedValue();
  const refetch = vi.spyOn(queryClient, "refetchQueries").mockResolvedValue();
  queryClient.setQueryData<Configuration>(["configuration", "list"], configuration(4, "current"));
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  const { result } = renderHook(
    () =>
      useConfigurationMutationLifecycle<Configuration>({
        cacheKey: ["configuration", "list"],
        invalidateKey: ["configuration"],
        refreshKey: ["configuration", "list"],
      }),
    { wrapper },
  );

  act(() => result.current.publish(configuration(3, "stale")));
  expect(queryClient.getQueryData(["configuration", "list"])).toEqual(
    configuration(4, "current"),
  );

  act(() => result.current.publish(configuration(5, "fresh")));
  expect(queryClient.getQueryData(["configuration", "list"])).toEqual(
    configuration(5, "fresh"),
  );
  expect(invalidate).toHaveBeenLastCalledWith({ queryKey: ["configuration"] });

  await act(() => result.current.refreshAfterFailure());
  expect(refetch).toHaveBeenCalledWith({
    queryKey: ["configuration", "list"],
    type: "active",
  });
});

test("does not invent an invalidation scope when a feature omits one", () => {
  const queryClient = new QueryClient();
  const invalidate = vi.spyOn(queryClient, "invalidateQueries").mockResolvedValue();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  const { result } = renderHook(
    () =>
      useConfigurationMutationLifecycle<Configuration>({
        cacheKey: ["oauth", "accounts"],
        refreshKey: ["oauth", "accounts"],
      }),
    { wrapper },
  );

  act(() => result.current.publish(configuration(1, "account")));

  expect(queryClient.getQueryData(["oauth", "accounts"])).toEqual(configuration(1, "account"));
  expect(invalidate).not.toHaveBeenCalled();
});

function configuration(configRevision: number, value: string): Configuration {
  return { configRevision, value };
}
