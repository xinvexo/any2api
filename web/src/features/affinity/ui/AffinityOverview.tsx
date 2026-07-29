import { Link } from "react-router-dom";

import { getAffinityErrorMessage } from "../model/affinity-error";
import { useAffinity } from "../model/use-affinity";

export function AffinityOverview() {
  const query = useAffinity();

  if (query.isPending && !query.data) {
    return (
      <section className="text-sm text-secondary lg:pl-8" aria-busy="true">
        正在读取活动会话汇总
      </section>
    );
  }

  if (!query.data) {
    return (
      <section className="lg:pl-8" role="alert">
        <h2 className="text-base font-semibold tracking-tight">活动会话</h2>
        <p className="mt-2 text-sm leading-6 text-secondary">
          {getAffinityErrorMessage(query.error)}
        </p>
      </section>
    );
  }

  const runtime = query.data;
  return (
    <section className="min-w-0 lg:pl-8" aria-busy={query.isFetching}>
      <header className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <h2 className="text-base font-semibold tracking-tight">活动会话</h2>
          <p className="mt-1.5 text-sm leading-6 text-secondary">
            {runtime.affinityEnabled
              ? "统计有效期内仍会命中的显式会话，不包含 Response ID 续接索引。"
              : "会话粘性已关闭；Response ID 续接索引不计入会话数。"}
          </p>
        </div>
        <Link
          to="/settings/routing"
          className="focus-ring shrink-0 rounded-[7px] px-2.5 py-1.5 text-xs font-medium text-secondary transition-colors hover:bg-surface-muted hover:text-primary"
        >
          调整策略
        </Link>
      </header>

      {query.isError ? (
        <p className="mt-4 border-l-2 border-warning pl-3 text-xs leading-5 text-secondary" role="status">
          刷新失败，仍显示最近数据：{getAffinityErrorMessage(query.error)}
        </p>
      ) : null}

      <dl className="mt-5 grid gap-3 sm:grid-cols-2 lg:grid-cols-1">
        <Metric label="当前活动" value={runtime.activeSessionCount} />
        <Metric label="正在建立" value={runtime.creatingSessionCount} />
      </dl>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <div className="min-w-0 rounded-[12px] bg-surface-muted px-4 py-4">
      <dt className="text-xs font-medium text-secondary">{label}</dt>
      <dd className="mt-2 text-[1.75rem] font-semibold tracking-tight tabular-nums">
        {value.toLocaleString("zh-CN")}
      </dd>
    </div>
  );
}
