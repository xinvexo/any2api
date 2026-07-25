import { Link } from "react-router-dom";

import { getBalancingErrorMessage } from "../model/balancing-error";
import { useBalancingRuntime } from "../model/use-balancing-runtime";
import { BalancingSummary } from "./BalancingSummary";
import { Surface } from "@/shared/ui/Surface";

export function BalancingOverview() {
  const query = useBalancingRuntime();

  if (query.isPending && !query.data) {
    return (
      <Surface className="p-5 text-sm text-secondary" aria-busy="true">
        正在读取请求调度汇总
      </Surface>
    );
  }

  if (!query.data) {
    return (
      <Surface className="p-5" role="alert">
        <h2 className="font-semibold">请求调度</h2>
        <p className="mt-2 text-sm text-secondary">{getBalancingErrorMessage(query.error)}</p>
      </Surface>
    );
  }

  const runtime = query.data;
  return (
    <Surface className="overflow-hidden" aria-busy={query.isFetching}>
      <header className="flex items-start justify-between gap-4 px-5 py-4">
        <div>
          <h2 className="font-semibold">请求调度</h2>
          <p className="mt-1 text-xs leading-5 text-secondary">
            配置版本 {runtime.configRevision} · 调度 Epoch {runtime.schedulerEpoch}
          </p>
        </div>
        <Link
          to="/settings/routing"
          className="focus-ring shrink-0 rounded-[7px] px-2.5 py-1.5 text-xs font-medium text-secondary hover:bg-surface-muted hover:text-primary"
        >
          调整策略
        </Link>
      </header>

      {query.isError ? (
        <p className="border-t border-warning/30 bg-warning/5 px-5 py-2 text-xs text-secondary" role="status">
          刷新失败，仍显示最近数据：{getBalancingErrorMessage(query.error)}
        </p>
      ) : null}

      <BalancingSummary runtime={runtime} />
    </Surface>
  );
}
