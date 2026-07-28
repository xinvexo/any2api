import { Link } from "react-router-dom";

import { getBalancingErrorMessage } from "../model/balancing-error";
import { useBalancingRuntime } from "../model/use-balancing-runtime";
import { BalancingSummary } from "./BalancingSummary";

export function BalancingOverview() {
  const query = useBalancingRuntime();

  if (query.isPending && !query.data) {
    return (
      <section className="py-6 text-sm text-secondary lg:pr-6" aria-busy="true">
        正在读取请求调度汇总
      </section>
    );
  }

  if (!query.data) {
    return (
      <section className="py-6 lg:pr-6" role="alert">
        <h2 className="font-semibold">请求调度</h2>
        <p className="mt-2 text-sm text-secondary">{getBalancingErrorMessage(query.error)}</p>
      </section>
    );
  }

  const runtime = query.data;
  return (
    <section className="min-w-0 py-6 lg:pr-6" aria-busy={query.isFetching}>
      <header className="flex items-start justify-between gap-4">
        <div>
          <h2 className="font-semibold">请求调度</h2>
          <p className="mt-1 text-xs leading-5 text-secondary">
            调度 Epoch {runtime.schedulerEpoch}
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
        <p className="mt-3 border-l-2 border-warning pl-3 text-xs text-secondary" role="status">
          刷新失败，仍显示最近数据：{getBalancingErrorMessage(query.error)}
        </p>
      ) : null}

      <BalancingSummary runtime={runtime} />
    </section>
  );
}
