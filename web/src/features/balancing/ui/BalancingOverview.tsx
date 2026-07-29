import { getBalancingErrorMessage } from "../model/balancing-error";
import { useBalancingRuntime } from "../model/use-balancing-runtime";
import { BalancingSummary } from "./BalancingSummary";

export function BalancingOverview() {
  const query = useBalancingRuntime();

  if (query.isPending && !query.data) {
    return (
      <section className="text-sm text-secondary" aria-busy="true">
        正在读取请求调度汇总
      </section>
    );
  }

  if (!query.data) {
    return (
      <section role="alert">
        <h2 className="text-base font-semibold tracking-tight">请求调度</h2>
        <p className="mt-2 text-sm leading-6 text-secondary">
          {getBalancingErrorMessage(query.error)}
        </p>
      </section>
    );
  }

  const runtime = query.data;
  return (
    <section className="min-w-0" aria-busy={query.isFetching}>
      <header>
        <h2 className="text-base font-semibold tracking-tight">请求调度</h2>
      </header>

      {query.isError ? (
        <p className="mt-4 border-l-2 border-warning pl-3 text-xs leading-5 text-secondary" role="status">
          刷新失败，仍显示最近数据：{getBalancingErrorMessage(query.error)}
        </p>
      ) : null}

      <BalancingSummary runtime={runtime} />
    </section>
  );
}
