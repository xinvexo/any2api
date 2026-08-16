import { CheckCircle2, LoaderCircle, RefreshCw, ServerCrash } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import { useBalancingRuntime, type BalancingRuntime } from "@/features/balancing";
import { isOverviewUsageRange, useOverviewUsage } from "@/features/overview-usage";
import { cn } from "@/shared/lib/cn";
import { notify } from "@/shared/notifications";
import { IconButton } from "@/shared/ui/IconButton";

import { useOverviewResources } from "../model/use-overview-resources";
import { LiveLoadPanel } from "./LiveLoadPanel";
import { LiveResourceGrid } from "./LiveResourceGrid";

type SystemStatus = "pending" | "error" | "ok" | "draining" | "forced";

export function SystemOverview() {
  const [searchParams] = useSearchParams();
  const rangeParam = searchParams.get("range");
  const range = isOverviewUsageRange(rangeParam) ? rangeParam : "24h";
  const runtime = useBalancingRuntime();
  const resources = useOverviewResources();
  const usage = useOverviewUsage(range);
  const [manualRefreshing, setManualRefreshing] = useState(false);
  const status = resolveSystemStatus(
    runtime.isPending,
    runtime.isError,
    runtime.data?.process.shutdownPhase,
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
      <header className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-2">
            <h1 className="text-[1.7rem] font-semibold leading-tight tracking-tight sm:text-2xl">
              系统总览
            </h1>
            <StatusBadge status={status} />
          </div>
          <p className="mt-1.5 text-xs text-tertiary">进程、主机与调用质量</p>
        </div>
        <IconButton
          label={manualRefreshing ? "刷新中" : "刷新系统总览"}
          title={manualRefreshing ? "刷新中" : "刷新系统总览"}
          onClick={() => void refresh()}
          disabled={manualRefreshing}
          className="mt-0.5"
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

      {runtime.data?.providers.length ? <ProviderLoadSummary runtime={runtime.data} /> : null}
    </section>
  );
}

function resolveSystemStatus(
  pending: boolean,
  error: boolean,
  phase: "running" | "draining" | "forced" | undefined,
): SystemStatus {
  if (pending) return "pending";
  if (error) return "error";
  return phase === "draining" || phase === "forced" ? phase : "ok";
}

function StatusBadge({ status }: { status: SystemStatus }) {
  const label = {
    pending: "正在连接",
    error: "连接失败",
    ok: "运行正常",
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
        status === "pending" && "bg-surface-muted text-secondary",
      )}
      role="status"
      aria-live="polite"
    >
      {status === "pending" ? (
        <LoaderCircle size={13} className="animate-spin" aria-hidden="true" />
      ) : status === "error" || status === "forced" ? (
        <ServerCrash size={13} aria-hidden="true" />
      ) : status === "draining" ? (
        <LoaderCircle size={13} aria-hidden="true" />
      ) : (
        <CheckCircle2 size={13} aria-hidden="true" />
      )}
      {label}
    </span>
  );
}

function ProviderLoadSummary({ runtime }: { runtime: BalancingRuntime }) {
  return (
    <section className="mt-8 border-t border-subtle pt-5" aria-labelledby="provider-load-title">
      <div className="flex items-baseline justify-between gap-4">
        <h2 id="provider-load-title" className="text-sm font-semibold tracking-tight">
          Provider 负载
        </h2>
        <p className="text-[11px] tabular-nums text-tertiary">近 60 秒</p>
      </div>
      <ul className="mt-3 grid min-w-0 gap-2 sm:grid-cols-2">
        {runtime.providers.map((provider) => (
          <li
            key={provider.providerKind}
            className="flex min-w-0 items-center justify-between gap-4 rounded-[8px] border border-subtle bg-surface/55 px-3.5 py-3"
          >
            <span className="truncate text-sm font-medium">{providerLabel(provider.providerKind)}</span>
            <span className="shrink-0 text-right text-[11px] tabular-nums text-secondary">
              {formatCount(provider.requestsInWindow)} 次 · {formatCount(provider.inFlight)} 活动
              {provider.limitedCredentialCount > 0
                ? ` · ${formatCount(provider.rateLimitedCredentialCount)}/${formatCount(provider.limitedCredentialCount)} RPM 用尽`
                : ""}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}

function formatCount(value: number) {
  return value.toLocaleString("zh-CN");
}

function providerLabel(provider: BalancingRuntime["providers"][number]["providerKind"]) {
  return { codex: "Codex", claude: "Claude", grok: "Grok", kimi: "Kimi" }[provider];
}
