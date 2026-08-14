import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { PropsWithChildren } from "react";

import { useOAuthLogin } from "./use-oauth-login";

afterEach(() => {
  vi.unstubAllGlobals();
});

test("ignores an older login start after a new provider flow begins", async () => {
  let resolveCodex: (response: Response) => void = () => undefined;
  let resolveClaude: (response: Response) => void = () => undefined;
  const codexResponse = new Promise<Response>((resolve) => {
    resolveCodex = resolve;
  });
  const claudeResponse = new Promise<Response>((resolve) => {
    resolveClaude = resolve;
  });
  vi.stubGlobal(
    "fetch",
    vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
      const provider = JSON.parse(String(init?.body)).provider as string;
      return provider === "codex" ? codexResponse : claudeResponse;
    }),
  );

  const { result } = renderLoginHook();
  let first!: ReturnType<typeof result.current.start>;
  let second!: ReturnType<typeof result.current.start>;
  act(() => {
    first = result.current.start("codex", { mode: "global" });
  });
  act(() => {
    second = result.current.start("claude", { mode: "global" });
  });

  await act(async () => {
    resolveCodex(startResponse("codex", "old-session"));
    await first;
  });
  expect(result.current.session).toBeNull();
  expect(result.current.pending).toBe("start");

  await act(async () => {
    resolveClaude(startResponse("claude", "current-session"));
    await second;
  });
  expect(result.current.session).toMatchObject({
    provider: "claude",
    sessionId: "current-session",
  });
  expect(result.current.pending).toBeNull();
});

test("reconciles the account list after an indeterminate exchange failure", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
    if (String(input).endsWith("/oauth/start")) {
      return startResponse("codex", "session-1");
    }
    throw new TypeError("connection closed after commit");
  });
  vi.stubGlobal("fetch", fetchMock);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const invalidate = vi.spyOn(client, "invalidateQueries");
  const { result } = renderLoginHook(client);

  await act(async () => {
    await result.current.start("codex", { mode: "global" });
  });
  await act(async () => {
    await expect(
      result.current.exchange("http://localhost:1455/auth/callback?code=x&state=y"),
    ).rejects.toThrow("connection closed after commit");
  });

  expect(invalidate).toHaveBeenCalledWith({
    queryKey: ["oauth", "accounts"],
    refetchType: "active",
  });
  await waitFor(() => expect(result.current.session).toBeNull());
});

function renderLoginHook(client = new QueryClient()) {
  const Wrapper = ({ children }: PropsWithChildren) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return renderHook(() => useOAuthLogin(), { wrapper: Wrapper });
}

function startResponse(provider: "codex" | "claude", sessionId: string) {
  return new Response(
    JSON.stringify({
      flow: "authorization_code",
      provider,
      session_id: sessionId,
      authorization_url: "https://auth.example.com/authorize",
      redirect_uri: "http://localhost:1455/auth/callback",
      expires_in_seconds: 600,
    }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}
