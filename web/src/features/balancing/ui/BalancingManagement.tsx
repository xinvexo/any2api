import { RefreshCw } from "lucide-react";

import { getBalancingErrorMessage } from "../model/balancing-error";
import { useBalancingRuntime } from "../model/use-balancing-runtime";
import { BalancingSummary } from "./BalancingSummary";
import { CredentialBalancingList } from "./CredentialBalancingList";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function BalancingManagement() {
  const query = useBalancingRuntime();
  if (query.isPending && !query.data) return <Surface className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary" aria-busy="true">正在读取负载均衡运行态</Surface>;
  if (!query.data) return (
    <Surface className="p-6" role="alert">
      <p className="font-semibold">无法读取负载均衡运行态</p>
      <p className="mt-2 text-sm text-secondary">{getBalancingErrorMessage(query.error)}</p>
      <Button className="mt-5" onClick={() => void query.refetch()} disabled={query.isFetching}><RefreshCw size={15} />重试</Button>
    </Surface>
  );
  const runtime = query.data;
  return (
    <div className="space-y-5" aria-busy={query.isFetching}>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-sm text-secondary">配置版本 <span className="font-medium tabular-nums text-primary">{runtime.configRevision}</span><span className="mx-2 text-tertiary">·</span>调度 Epoch <span className="font-medium tabular-nums text-primary">{runtime.schedulerEpoch}</span></p>
        <Button variant="ghost" onClick={() => void query.refetch()} disabled={query.isFetching}><RefreshCw size={15} className={query.isFetching ? "animate-spin" : undefined} />刷新</Button>
      </div>
      {query.isError ? <Surface className="border-warning/40 p-4 text-sm text-secondary" role="status">刷新失败，当前仍显示最近一次有效数据：{getBalancingErrorMessage(query.error)}</Surface> : null}
      <BalancingSummary runtime={runtime} />
      <CredentialBalancingList credentials={runtime.credentials} />
      <p className="text-xs leading-5 text-tertiary">选中与过滤次数只属于当前进程；等待重选可能让过滤次数高于客户端请求数，不能用于计费或配额。</p>
    </div>
  );
}
