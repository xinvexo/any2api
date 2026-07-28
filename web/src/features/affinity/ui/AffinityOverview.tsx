import { Link2, LoaderCircle } from "lucide-react";
import { Link } from "react-router-dom";

import { getAffinityErrorMessage } from "../model/affinity-error";
import { useAffinity } from "../model/use-affinity";

export function AffinityOverview() {
  const query = useAffinity();

  if (query.isPending && !query.data) {
    return (
      <section className="border-t border-subtle py-6 text-sm text-secondary lg:border-t-0 lg:pl-6" aria-busy="true">
        正在读取会话绑定汇总
      </section>
    );
  }

  if (!query.data) {
    return (
      <section className="border-t border-subtle py-6 lg:border-t-0 lg:pl-6" role="alert">
        <h2 className="font-semibold">会话绑定</h2>
        <p className="mt-2 text-sm text-secondary">{getAffinityErrorMessage(query.error)}</p>
      </section>
    );
  }

  const runtime = query.data;
  return (
    <section className="min-w-0 border-t border-subtle py-6 lg:border-t-0 lg:pl-6" aria-busy={query.isFetching}>
      <header className="flex items-start justify-between gap-4">
        <div>
          <h2 className="font-semibold">会话绑定</h2>
          <p className="mt-1 text-xs leading-5 text-secondary">只统计当前进程，重启后自动清空。</p>
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
          刷新失败，仍显示最近数据：{getAffinityErrorMessage(query.error)}
        </p>
      ) : null}

      <dl className="mt-5 grid border-y border-subtle sm:grid-cols-2 lg:grid-cols-1">
        <Metric icon={Link2} label="当前绑定" value={runtime.bindingCount} />
        <Metric icon={LoaderCircle} label="正在创建" value={runtime.creatingCount} />
      </dl>
    </section>
  );
}

function Metric({ icon: Icon, label, value }: { icon: typeof Link2; label: string; value: number }) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-subtle px-3 py-3.5 last:border-b-0 sm:border-b-0 sm:border-r sm:last:border-r-0 sm:px-4 lg:border-b lg:border-r-0 lg:last:border-b-0">
      <div>
        <dt className="text-xs text-secondary">{label}</dt>
        <dd className="mt-1 text-xl font-semibold tabular-nums">{value.toLocaleString("zh-CN")}</dd>
      </div>
      <Icon size={17} className="text-tertiary" aria-hidden="true" />
    </div>
  );
}
