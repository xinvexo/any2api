import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

const probes = vi.hoisted(() => ({
  runtime: undefined as RuntimeQuery | undefined,
  resources: undefined as ResourcesQuery | undefined,
  usage: undefined as UsageQuery | undefined,
  realtime: { connected: true, stale: false },
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
vi.mock("@/shared/realtime", () => ({
  useAdminRealtimeStatus: () => probes.realtime,
}));

import { SystemOverview } from "./SystemOverview";

afterEach(() => {
  vi.restoreAllMocks();
  probes.realtime = { connected: true, stale: false };
});

test("shows resource and request load bands, then refreshes all overview queries", async () => {
  const refetch = vi.fn(async () => ({ isSuccess: true }));
  probes.runtime = runtimeQuery(refetch);
  probes.resources = resourcesQuery(refetch);
  probes.usage = usageQuery(refetch);

  const { container } = render(
    <MemoryRouter initialEntries={["/overview?range=24h"]}>
      <SystemOverview />
    </MemoryRouter>,
  );

  expect(screen.getByText("ANY2API 内存")).toBeInTheDocument();
  expect(screen.getByText("ANY2API CPU")).toBeInTheDocument();
  expect(screen.getByText("整机内存")).toBeInTheDocument();
  expect(screen.getByText("正文映射内存")).toBeInTheDocument();
  expect(screen.getByText("内存回收阻塞")).toBeInTheDocument();
  expect(screen.getByText("进行中请求")).toBeInTheDocument();
  expect(screen.getByText("近 60 秒请求")).toBeInTheDocument();
  expect(screen.getByText("账号与密钥")).toBeInTheDocument();
  expect(screen.getByText("已达每分钟上限")).toBeInTheDocument();
  expect(screen.queryByRole("progressbar", { name: "近 60 秒请求 使用率" }))
    .not.toBeInTheDocument();
  expect(screen.queryByRole("progressbar", { name: "进行中请求 使用率" }))
    .not.toBeInTheDocument();
  expect(screen.getByText("请求负载")).toBeInTheDocument();
  expect(screen.getByText("资源状态")).toBeInTheDocument();
  expect(screen.getByText("运行正常")).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "系统总览" })).not.toBeInTheDocument();
  expect(screen.queryByText("进程、主机与调用质量")).not.toBeInTheDocument();
  expect(screen.queryByText("实时")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "刷新系统总览" })).toBeInTheDocument();
  expect(screen.queryByText("后台任务")).not.toBeInTheDocument();
  expect(screen.queryByText("缓存命中")).not.toBeInTheDocument();
  expect(container).not.toHaveTextContent(/活动上游|Transport 客户端|逻辑 CPU|RSS/);
  expect(container.querySelector("time")).toBeNull();

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

test("does not show an account limit warning when no limited credential is exhausted", () => {
  const refetch = vi.fn(async () => ({ isSuccess: true }));
  probes.runtime = runtimeQuery(refetch);
  probes.runtime.data.totals.rateLimitedCredentialCount = 0;
  probes.resources = resourcesQuery(refetch);
  probes.usage = usageQuery(refetch);

  render(
    <MemoryRouter>
      <SystemOverview />
    </MemoryRouter>,
  );

  expect(screen.queryByText("已达每分钟上限")).not.toBeInTheDocument();
});

test("keeps the last snapshot visible and marks a disconnected stream stale", () => {
  const refetch = vi.fn(async () => ({ isSuccess: true }));
  probes.runtime = runtimeQuery(refetch);
  probes.resources = resourcesQuery(refetch);
  probes.usage = usageQuery(refetch);
  probes.realtime = { connected: false, stale: true };

  render(
    <MemoryRouter>
      <SystemOverview />
    </MemoryRouter>,
  );

  expect(screen.getByText("128 MiB")).toBeInTheDocument();
  expect(screen.getByText("数据陈旧")).toBeInTheDocument();
  expect(screen.getByText("实时连接已中断，仍显示最近一次有效快照。"))
    .toHaveAttribute("role", "status");
});

interface RuntimeQuery {
  data: {
    process: { shutdownPhase: "running"; activeRequests: number; backgroundTasks: number };
    queue: { waiting: number; maxWaiting: number };
    totals: {
      credentialCount: number;
      enabledCredentialCount: number;
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
    ownership: {
      payloadBuffers: {
        heapCurrentBytes: number;
        heapPeakBytes: number;
        mappedCurrentBytes: number;
        mappedPeakBytes: number;
        httpBodyCaptureCurrentBytes: number;
        httpBodyCapturePeakBytes: number;
      };
      telemetry: {
        queuedOwnedBytes: number;
        inFlightOwnedBytes: number;
        reservedOwnedBytes: number;
      };
      reclamation: { blockers: number; completedRuns: number; lastDurationMicros: number };
    };
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
      queue: { waiting: 2, maxWaiting: 128 },
      totals: {
        credentialCount: 64,
        enabledCredentialCount: 48,
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
      ownership: {
        payloadBuffers: {
          heapCurrentBytes: 1 * 1024 ** 2,
          heapPeakBytes: 2 * 1024 ** 2,
          mappedCurrentBytes: 8 * 1024 ** 2,
          mappedPeakBytes: 16 * 1024 ** 2,
          httpBodyCaptureCurrentBytes: 512 * 1024,
          httpBodyCapturePeakBytes: 1 * 1024 ** 2,
        },
        telemetry: {
          queuedOwnedBytes: 256 * 1024,
          inFlightOwnedBytes: 128 * 1024,
          reservedOwnedBytes: 384 * 1024,
        },
        reclamation: { blockers: 0, completedRuns: 12, lastDurationMicros: 725 },
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
