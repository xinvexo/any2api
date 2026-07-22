import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { afterEach, expect, test, vi } from "vitest";

import { SystemOverview } from "./SystemOverview";

function Wrapper({ children }: PropsWithChildren) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

afterEach(() => vi.restoreAllMocks());

test("renders live system status metrics", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    new Response(
      JSON.stringify({ status: "ok", config_revision: 7, scheduler_epoch: 2, shutdown_phase: "running", active_requests: 1, background_tasks: 2 }),
      { status: 200, headers: { "Content-Type": "application/json" } },
    ),
  );

  render(<SystemOverview />, { wrapper: Wrapper });

  expect(await screen.findByText("2")).toBeInTheDocument();
  expect(screen.getByText("运行正常")).toBeInTheDocument();
  expect(screen.getByText("运行中")).toBeInTheDocument();
  expect(screen.getByText("1 / 2")).toBeInTheDocument();
});

test("rejects an incompatible health payload", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    new Response(JSON.stringify({ status: "ok" }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    }),
  );

  render(<SystemOverview />, { wrapper: Wrapper });

  expect(await screen.findByText("连接失败")).toBeInTheDocument();
});
