import { CheckCircle2, LoaderCircle, RefreshCw, ServerCrash } from "lucide-react";

import { useHealth } from "../model/use-health";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function SystemOverview() {
  const health = useHealth();

  return (
    <Surface className="overflow-hidden" aria-busy={health.isFetching}>
      <div className="px-5 py-4 sm:px-6 sm:py-5">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-center gap-4">
            <span className="grid size-10 place-items-center rounded-control bg-surface-muted text-secondary">
              {health.isPending ? (
                <LoaderCircle size={21} className="animate-spin" />
              ) : health.isError ? (
                <ServerCrash size={21} className="text-danger" />
              ) : (
                <CheckCircle2 size={21} className="text-success" />
              )}
            </span>
            <div>
              <h2 className="text-sm font-semibold">服务状态</h2>
              <p className="mt-1 text-xs text-secondary" role="status" aria-live="polite">
                {health.isPending ? "正在连接" : health.isError ? "连接失败" : "运行正常"}
              </p>
            </div>
          </div>
          <Button variant="ghost" onClick={() => void health.refetch()} disabled={health.isFetching}>
            <RefreshCw size={16} className={health.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
        </div>
      </div>

      <dl className="grid border-t border-subtle sm:grid-cols-3">
        <Metric label="配置版本" value={health.data?.config_revision ?? "-"} />
        <Metric label="进程阶段" value={phaseLabel(health.data?.shutdown_phase)} />
        <Metric label="活动 / 后台任务" value={health.data ? `${health.data.active_requests} / ${health.data.background_tasks}` : "-"} />
      </dl>
    </Surface>
  );
}

function phaseLabel(phase: "running" | "draining" | "forced" | undefined) {
  if (phase === "draining") return "正在排空";
  if (phase === "forced") return "强制收尾";
  return phase === "running" ? "运行中" : "-";
}

function Metric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="border-b border-subtle px-5 py-4 last:border-b-0 sm:border-b-0 sm:border-r sm:last:border-r-0">
      <dt className="text-xs text-secondary">{label}</dt>
      <dd className="mt-1 text-lg font-semibold tabular-nums">{value}</dd>
    </div>
  );
}
