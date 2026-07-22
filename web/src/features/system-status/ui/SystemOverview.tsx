import { CheckCircle2, LoaderCircle, RefreshCw, ServerCrash } from "lucide-react";

import { useHealth } from "../model/use-health";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function SystemOverview() {
  const health = useHealth();

  return (
    <div className="space-y-5">
      <Surface className="p-5 sm:p-6" aria-busy={health.isFetching}>
        <div className="flex flex-col gap-5 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-center gap-4">
            <span className="grid size-11 place-items-center rounded-control bg-surface-muted text-secondary">
              {health.isPending ? (
                <LoaderCircle size={21} className="animate-spin" />
              ) : health.isError ? (
                <ServerCrash size={21} className="text-danger" />
              ) : (
                <CheckCircle2 size={21} className="text-success" />
              )}
            </span>
            <div>
              <p className="text-sm font-semibold">服务状态</p>
              <p className="mt-1 text-sm text-secondary" role="status" aria-live="polite">
                {health.isPending ? "正在连接" : health.isError ? "连接失败" : "运行正常"}
              </p>
            </div>
          </div>
          <Button variant="ghost" onClick={() => void health.refetch()} disabled={health.isFetching}>
            <RefreshCw size={16} className={health.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
        </div>
      </Surface>

      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Metric label="配置版本" value={health.data?.config_revision ?? "-"} />
        <Metric label="调度 Epoch" value={health.data?.scheduler_epoch ?? "-"} />
        <Metric label="进程阶段" value={phaseLabel(health.data?.shutdown_phase)} />
        <Metric label="活动 / 后台任务" value={health.data ? `${health.data.active_requests} / ${health.data.background_tasks}` : "-"} />
      </div>
    </div>
  );
}

function phaseLabel(phase: "running" | "draining" | "forced" | undefined) {
  if (phase === "draining") return "正在排空";
  if (phase === "forced") return "强制收尾";
  return phase === "running" ? "运行中" : "-";
}

function Metric({ label, value }: { label: string; value: number | string }) {
  return (
    <Surface className="min-h-28 p-5">
      <p className="text-sm text-secondary">{label}</p>
      <p className="mt-3 text-2xl font-semibold tabular-nums">{value}</p>
    </Surface>
  );
}
