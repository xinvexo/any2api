import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { expect, test, vi } from "vitest";

import { useProviderSecretActions } from "./use-provider-secret-actions";

const apiMocks = vi.hoisted(() => ({
  create: vi.fn(),
  rotate: vi.fn(),
}));

vi.mock("../api/provider-credential-api", () => ({
  createProviderCredential: apiMocks.create,
  rotateProviderCredential: apiMocks.rotate,
}));

test("does not finish credential creation before the authoritative cache refresh", async () => {
  const endpointId = "endpoint-1";
  apiMocks.create.mockResolvedValue({
    configRevision: 2,
    providerEndpointId: endpointId,
    items: [],
  });
  const refresh = deferred<void>();
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  vi.spyOn(client, "refetchQueries").mockImplementation(() => refresh.promise);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const { result } = renderHook(() => useProviderSecretActions(endpointId), { wrapper });
  let completed = false;

  const creation = result.current.create({
    expectedRevision: 1,
    label: "Primary",
    apiKey: "secret",
    proxyProfileId: "direct",
    requestsPerMinute: null,
    enabled: true,
  }).then(() => {
    completed = true;
  });

  await waitFor(() => expect(client.refetchQueries).toHaveBeenCalled());
  expect(completed).toBe(false);

  refresh.resolve();
  await act(async () => creation);
  expect(completed).toBe(true);
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
