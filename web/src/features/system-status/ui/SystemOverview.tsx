import { CheckCircle2, LoaderCircle, RefreshCw, ServerCrash } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import { useBalancingRuntime } from "@/features/balancing";
import { isOverviewUsageRange, useOverviewUsage } from "@/features/overview-usage";
import { cn } from "@/shared/lib/cn";
import { notify } from "@/shared/notifications";
import { useAdminRealtimeStatus } from "@/shared/realtime";
import { IconButton } from "@/shared/ui/IconButton";

import { useOverviewResources } from "../model/use-overview-resources";
import { LiveLoadPanel } from "./LiveLoadPanel";
import { LiveResourceGrid, MemoryOwnershipDetails } from "./LiveResourceGrid";

type SystemStatus = "pending" | "error" | "ok" | "stale" | "draining" | "forced";

export function SystemOverview() {
  const [searchParams] = useSearchParams();
  const rangeParam = searchParams.get("range");
  const range = isOverviewUsageRange(rangeParam) ? rangeParam : "24h";
  const runtime = useBalancingRuntime();
  const resources = useOverviewResources();
  const usage = useOverviewUsage(range);
  const realtime = useAdminRealtimeStatus();
  const [manualRefreshing, setManualRefreshing] = useState(false);
  const status = resolveSystemStatus(
    runtime.isPending,
    runtime.isError,
    runtime.data?.process.shutdownPhase,
    realtime.stale,
  );

  async function refresh() {
    if (manualRefreshing) return;
    setManualRefreshing(true);
    try {
      const results = await Promise.all([
        runtime.refetch(),
        resources.refetch(),
        usage.refetch(),
      ]);
      if (results.every((result) => result.isSuccess)) {
        notify.success("系统总览已刷新");
      }
    } finally {
      setManualRefreshing(false);
    }
  }

  return (
    <section
      className="min-w-0"
      aria-busy={runtime.isFetching || resources.isFetching || usage.isFetching}
    >
      <header className="flex min-h-8 items-center justify-between gap-4">
        <StatusBadge status={status} />
        <IconButton
          label={manualRefreshing ? "刷新中" : "刷新系统总览"}
          title={manualRefreshing ? "刷新中" : "刷新系统总览"}
          onClick={() => void refresh()}
          disabled={manualRefreshing}
        >
          <RefreshCw
            size={16}
            className={manualRefreshing ? "animate-spin" : undefined}
            aria-hidden="true"
          />
        </IconButton>
      </header>

      <div className="mt-5 grid min-w-0 gap-6 lg:grid-cols-2 lg:items-start">
        <LiveResourceGrid resources={resources.data} />
        <LiveLoadPanel runtime={runtime.data} />
        <MemoryOwnershipDetails ownership={resources.data?.ownership} />
      </div>

      {resources.isError ? (
        <p
          className="mt-4 border-l-2 border-warning pl-3 text-xs leading-5 text-secondary"
          role={resources.data ? "status" : "alert"}
        >
          {resources.data
            ? "资源刷新失败，仍显示最近一次采样。"
            : "实时资源暂不可用，请稍后重试。"}
        </p>
      ) : null}

      {runtime.isError && runtime.data ? (
        <p className="mt-4 border-l-2 border-warning pl-3 text-xs leading-5 text-secondary" role="status">
          调度负载刷新失败，仍显示最近一次快照。
        </p>
      ) : null}

      {realtime.stale && runtime.data ? (
        <p className="mt-4 border-l-2 border-warning pl-3 text-xs leading-5 text-secondary" role="status">
          {realtime.connected
            ? "实时采样暂不可用，仍显示最近一次有效快照。"
            : "实时连接已中断，仍显示最近一次有效快照。"}
        </p>
      ) : null}
    </section>
  );
}

function resolveSystemStatus(
  pending: boolean,
  error: boolean,
  phase: "running" | "draining" | "forced" | undefined,
  stale: boolean,
): SystemStatus {
  if (pending) return "pending";
  if (phase === "draining" || phase === "forced") return phase;
  if (error) return "error";
  return stale ? "stale" : "ok";
}

function StatusBadge({ status }: { status: SystemStatus }) {
  const label = {
    pending: "正在连接",
    error: "连接失败",
    ok: "运行正常",
    stale: "数据陈旧",
    draining: "正在排空",
    forced: "强制停机",
  }[status];
  return (
    <span
      className={cn(
        "inline-flex h-7 items-center gap-1.5 rounded-full px-2.5 text-xs font-medium",
        status === "ok" && "bg-success/10 text-success",
        (status === "error" || status === "forced") && "bg-danger/10 text-danger",
        status === "draining" && "bg-warning/12 text-warning",
        status === "stale" && "bg-warning/12 text-warning",
        status === "pending" && "bg-surface-muted text-secondary",
      )}
      role="status"
      aria-live="polite"
    >
      {status === "pending" ? (
        <LoaderCircle size={13} className="animate-spin" aria-hidden="true" />
      ) : status === "error" || status === "forced" ? (
        <ServerCrash size={13} aria-hidden="true" />
      ) : status === "draining" || status === "stale" ? (
        <LoaderCircle size={13} aria-hidden="true" />
      ) : (
        <CheckCircle2 size={13} aria-hidden="true" />
      )}
      {label}
    </span>
  );
}
