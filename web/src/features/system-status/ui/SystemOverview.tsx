import { CheckCircle2, LoaderCircle, RefreshCw, ServerCrash } from "lucide-react";

import {
  describeAffinityMetrics,
  type AffinityMetricPresentation,
  useAffinity,
} from "@/features/affinity";
import { useBalancingRuntime } from "@/features/balancing";
import { cn } from "@/shared/lib/cn";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";

export function SystemOverview() {
  const runtime = useBalancingRuntime();
  const affinity = useAffinity();
  const affinityMetrics = describeAffinityMetrics(affinity.data);
  const status = runtime.isPending ? "pending" : runtime.isError ? "error" : "ok";
  const busy = runtime.isFetching || affinity.isFetching;

  async function refresh() {
    const [runtimeResult, affinityResult] = await Promise.all([
      runtime.refetch(),
      affinity.refetch(),
    ]);
    if (runtimeResult.isSuccess && affinityResult.isSuccess) {
      notify.success("系统状态已刷新");
    }
  }

  return (
    <section className="min-w-0" aria-busy={busy}>
      <header className="flex items-center justify-between gap-4">
        <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-2">
          <h1 className="text-2xl font-semibold tracking-tight">系统总览</h1>
          <StatusBadge status={status} />
        </div>
        <Button variant="secondary" size="sm" onClick={() => void refresh()} disabled={busy}>
          <RefreshCw size={14} className={busy ? "animate-spin" : undefined} />
          刷新
        </Button>
      </header>

      <dl className="mt-5 grid gap-3 sm:grid-cols-3">
        <MetricCard
          label={affinityMetrics.active.label}
          value={affinityMetrics.active.value}
          note={affinityMetrics.active.note}
        />
        <MetricCard
          label={affinityMetrics.creating.label}
          value={affinityMetrics.creating.value}
          note={affinityMetrics.creating.note}
        />
        <MetricCard
          label="活动请求 / 后台任务"
          value={
            runtime.data
              ? `${runtime.data.process.activeRequests} / ${runtime.data.process.backgroundTasks}`
              : "—"
          }
        />
      </dl>
    </section>
  );
}

function StatusBadge({ status }: { status: "pending" | "error" | "ok" }) {
  const label = status === "pending" ? "正在连接" : status === "error" ? "连接失败" : "运行正常";
  return (
    <span
      className={cn(
        "inline-flex h-7 items-center gap-1.5 rounded-full px-2.5 text-xs font-medium",
        status === "ok" && "bg-success/10 text-success",
        status === "error" && "bg-danger/10 text-danger",
        status === "pending" && "bg-surface-muted text-secondary",
      )}
      role="status"
      aria-live="polite"
    >
      {status === "pending" ? (
        <LoaderCircle size={13} className="animate-spin" aria-hidden="true" />
      ) : status === "error" ? (
        <ServerCrash size={13} aria-hidden="true" />
      ) : (
        <CheckCircle2 size={13} aria-hidden="true" />
      )}
      {label}
    </span>
  );
}

function MetricCard({
  label,
  value,
  note,
}: Pick<AffinityMetricPresentation, "label" | "value"> & { note?: string }) {
  return (
    <div className="min-w-0 rounded-[12px] bg-surface-muted px-4 py-4">
      <dt className="text-xs font-medium text-secondary">{label}</dt>
      <dd className="mt-2 truncate text-[1.75rem] font-semibold tracking-tight tabular-nums" title={value}>
        {value}
      </dd>
      {note ? <p className="mt-1.5 text-[11px] leading-4 text-tertiary">{note}</p> : null}
    </div>
  );
}
