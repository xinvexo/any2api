import { Link2, LoaderCircle, LockKeyhole } from "lucide-react";
import { Link } from "react-router-dom";

import { getAffinityErrorMessage } from "../model/affinity-error";
import { useAffinity } from "../model/use-affinity";
import { Surface } from "@/shared/ui/Surface";

export function AffinityOverview() {
  const query = useAffinity();

  if (query.isPending && !query.data) {
    return (
      <Surface className="p-5 text-sm text-secondary" aria-busy="true">
        正在读取会话绑定汇总
      </Surface>
    );
  }

  if (!query.data) {
    return (
      <Surface className="p-5" role="alert">
        <h2 className="font-semibold">会话绑定</h2>
        <p className="mt-2 text-sm text-secondary">{getAffinityErrorMessage(query.error)}</p>
      </Surface>
    );
  }

  const runtime = query.data;
  return (
    <Surface className="overflow-hidden" aria-busy={query.isFetching}>
      <header className="flex items-start justify-between gap-4 px-5 py-4">
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
        <p className="border-t border-warning/30 bg-warning/5 px-5 py-2 text-xs text-secondary" role="status">
          刷新失败，仍显示最近数据：{getAffinityErrorMessage(query.error)}
        </p>
      ) : null}

      <dl className="grid border-t border-subtle sm:grid-cols-3">
        <Metric icon={Link2} label="软绑定" value={runtime.softBindingCount} />
        <Metric icon={LockKeyhole} label="硬绑定" value={runtime.hardBindingCount} />
        <Metric icon={LoaderCircle} label="正在创建" value={runtime.creatingCount} />
      </dl>
    </Surface>
  );
}

function Metric({ icon: Icon, label, value }: { icon: typeof Link2; label: string; value: number }) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-subtle px-5 py-4 last:border-b-0 sm:border-b-0 sm:border-r sm:last:border-r-0">
      <div>
        <dt className="text-xs text-secondary">{label}</dt>
        <dd className="mt-1 text-xl font-semibold tabular-nums">{value.toLocaleString("zh-CN")}</dd>
      </div>
      <Icon size={17} className="text-tertiary" aria-hidden="true" />
    </div>
  );
}
