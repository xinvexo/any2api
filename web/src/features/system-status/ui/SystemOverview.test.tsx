import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

const probes = vi.hoisted(() => ({
  runtime: undefined as RuntimeQuery | undefined,
  resources: undefined as ResourcesQuery | undefined,
  usage: undefined as UsageQuery | undefined,
}));

vi.mock("@/features/balancing", () => ({
  useBalancingRuntime: () => probes.runtime,
}));
vi.mock("@/features/overview-usage", () => ({
  isOverviewUsageRange: (value: string | null): value is "1h" | "24h" | "7d" | "30d" =>
    value === "1h" || value === "24h" || value === "7d" || value === "30d",
  useOverviewUsage: () => probes.usage,
}));
vi.mock("../model/use-overview-resources", () => ({
  useOverviewResources: () => probes.resources,
}));

import { SystemOverview } from "./SystemOverview";

afterEach(() => {
  vi.restoreAllMocks();
});

test("shows resource and request load bands, then refreshes all overview queries", async () => {
  const refetch = vi.fn(async () => ({ isSuccess: true }));
  probes.runtime = runtimeQuery(refetch);
  probes.resources = resourcesQuery(refetch);
  probes.usage = usageQuery(refetch);

  render(
    <MemoryRouter initialEntries={["/overview?range=24h"]}>
      <SystemOverview />
    </MemoryRouter>,
  );

  expect(screen.getByText("any2api 内存")).toBeInTheDocument();
  expect(screen.getByText("any2api CPU")).toBeInTheDocument();
  expect(screen.getByText("系统内存")).toBeInTheDocument();
  expect(screen.getByText("活动上游")).toBeInTheDocument();
  expect(screen.getByText("近 60 秒请求")).toBeInTheDocument();
  expect(screen.getByText("客户端池条目")).toBeInTheDocument();
  expect(screen.getByText("请求负载")).toBeInTheDocument();
  expect(screen.getByText("资源状态")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "刷新系统总览" })).toBeInTheDocument();
  expect(screen.queryByText("后台任务")).not.toBeInTheDocument();
  expect(screen.queryByText("缓存命中")).not.toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "刷新系统总览" }));
  await waitFor(() => expect(refetch).toHaveBeenCalledTimes(3));
});

test("keeps the last resource values visible when a refresh fails", () => {
  probes.runtime = runtimeQuery(vi.fn());
  probes.resources = {
    ...resourcesQuery(vi.fn()),
    isError: true,
  };
  probes.usage = usageQuery(vi.fn());

  render(
    <MemoryRouter>
      <SystemOverview />
    </MemoryRouter>,
  );

  expect(screen.getByText("128 MiB")).toBeInTheDocument();
  expect(screen.getByText("资源刷新失败，仍显示最近一次采样。"))
    .toHaveAttribute("role", "status");
});

interface RuntimeQuery {
  data: {
    process: { shutdownPhase: "running"; activeRequests: number; backgroundTasks: number };
    transport: { cacheEntries: number; cacheCapacity: number };
    queue: { waiting: number; maxWaiting: number };
    totals: {
      inFlight: number;
      requestsInWindow: number;
      limitedCredentialCount: number;
      rateLimitedCredentialCount: number;
    };
    providers: Array<{
      providerKind: "codex";
      requestsInWindow: number;
      inFlight: number;
      limitedCredentialCount: number;
      rateLimitedCredentialCount: number;
    }>;
  };
  isPending: boolean;
  isError: boolean;
  isFetching: boolean;
  refetch: () => Promise<{ isSuccess: boolean }>;
}

interface ResourcesQuery {
  data: {
    sampledAtMs: number;
    process: { residentMemoryBytes: number; cpuUsagePercent: number };
    system: { usedMemoryBytes: number; totalMemoryBytes: number; cpuUsagePercent: number };
  };
  isError: boolean;
  isFetching: boolean;
  refetch: () => Promise<{ isSuccess: boolean }>;
}

interface UsageQuery {
  refetch: () => Promise<{ isSuccess: boolean }>;
}

function runtimeQuery(refetch: () => Promise<{ isSuccess: boolean }>): RuntimeQuery {
  return {
    data: {
      process: { shutdownPhase: "running", activeRequests: 1, backgroundTasks: 0 },
      transport: { cacheEntries: 3, cacheCapacity: 64 },
      queue: { waiting: 2, maxWaiting: 128 },
      totals: {
        inFlight: 1,
        requestsInWindow: 14,
        limitedCredentialCount: 2,
        rateLimitedCredentialCount: 1,
      },
      providers: [
        {
          providerKind: "codex",
          requestsInWindow: 14,
          inFlight: 1,
          limitedCredentialCount: 2,
          rateLimitedCredentialCount: 1,
        },
      ],
    },
    isPending: false,
    isError: false,
    isFetching: false,
    refetch,
  };
}

function resourcesQuery(refetch: () => Promise<{ isSuccess: boolean }>): ResourcesQuery {
  return {
    data: {
      sampledAtMs: 1,
      process: { residentMemoryBytes: 128 * 1024 ** 2, cpuUsagePercent: 2.5 },
      system: {
        usedMemoryBytes: 8 * 1024 ** 3,
        totalMemoryBytes: 16 * 1024 ** 3,
        cpuUsagePercent: 31.7,
      },
    },
    isError: false,
    isFetching: false,
    refetch,
  };
}

function usageQuery(refetch: () => Promise<{ isSuccess: boolean }>): UsageQuery {
  return { refetch };
}
