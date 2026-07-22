import { RefreshCw, Trash2 } from "lucide-react";

import { getAffinityErrorMessage } from "../model/affinity-error";
import { useAffinityMutations } from "../model/use-affinity-mutations";
import { useAffinity } from "../model/use-affinity";
import { AffinityBindings } from "./AffinityBindings";
import { AffinityMetrics } from "./AffinityMetrics";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function AffinityManagement() {
  const affinity = useAffinity();
  const mutations = useAffinityMutations();
  const pendingCredentialId = mutations.clearCredential.isPending
    ? mutations.clearCredential.variables
    : null;

  if (affinity.isPending && !affinity.data) {
    return (
      <Surface className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary" aria-busy="true">
        正在读取会话绑定
      </Surface>
    );
  }
  if (!affinity.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取会话绑定</p>
        <p className="mt-2 text-sm text-secondary">{getAffinityErrorMessage(affinity.error)}</p>
        <Button className="mt-5" onClick={() => void affinity.refetch()} disabled={affinity.isFetching}>
          <RefreshCw size={15} />
          重试
        </Button>
      </Surface>
    );
  }

  const runtime = affinity.data;

  function clearAll() {
    if (window.confirm("清除当前进程中的全部软绑定和硬绑定？正在进行的请求不会被终止。")) {
      mutations.clearAll.mutate();
    }
  }

  return (
    <div className="space-y-5" aria-busy={affinity.isFetching || mutations.isPending}>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-sm text-secondary">运行态仅保存在当前进程内</p>
        <div className="flex flex-col gap-2 sm:flex-row">
          <Button variant="ghost" onClick={() => void affinity.refetch()} disabled={affinity.isFetching}>
            <RefreshCw size={15} className={affinity.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
          <Button
            variant="danger"
            onClick={clearAll}
            disabled={mutations.isPending || runtime.softBindingCount + runtime.hardBindingCount === 0}
          >
            <Trash2 size={15} />
            {mutations.clearAll.isPending ? "正在清除" : "清除全部"}
          </Button>
        </div>
      </div>

      {affinity.isError ? (
        <Surface className="border-warning/40 p-4 text-sm text-secondary" role="status">
          运行态刷新失败，当前仍显示最近一次有效数据：{getAffinityErrorMessage(affinity.error)}
        </Surface>
      ) : null}
      {mutations.clearAll.error || mutations.clearCredential.error ? (
        <Surface className="border-danger/40 p-4 text-sm text-secondary" role="alert">
          {getAffinityErrorMessage(mutations.clearAll.error ?? mutations.clearCredential.error)}
        </Surface>
      ) : null}

      <AffinityMetrics
        soft={runtime.softBindingCount}
        hard={runtime.hardBindingCount}
        creating={runtime.creatingCount}
        credentials={runtime.credentialCounts}
        pendingCredentialId={pendingCredentialId ?? null}
        onClearCredential={(credentialId) => mutations.clearCredential.mutate(credentialId)}
      />
      <AffinityBindings bindings={runtime.bindings} />
    </div>
  );
}
