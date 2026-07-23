import { RefreshCw } from "lucide-react";

import { getProxyErrorMessage } from "../model/proxy-error";
import { useProxies } from "../model/use-proxies";
import { useProxyMutations } from "../model/use-proxy-mutations";
import { GlobalProxyPanel } from "./GlobalProxyPanel";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function GlobalProxySettings() {
  const proxies = useProxies();
  const mutations = useProxyMutations();

  if (proxies.isPending && !proxies.data) {
    return (
      <Surface className="flex min-h-28 items-center justify-center p-6 text-sm text-secondary" aria-busy="true">
        正在读取全局出口代理
      </Surface>
    );
  }

  if (!proxies.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取全局出口代理</p>
        <p className="mt-2 text-sm text-secondary">{getProxyErrorMessage(proxies.error)}</p>
        <Button className="mt-4" onClick={() => void proxies.refetch()} disabled={proxies.isFetching}>
          <RefreshCw size={15} className={proxies.isFetching ? "animate-spin" : undefined} />
          重试
        </Button>
      </Surface>
    );
  }

  return (
    <GlobalProxyPanel
      key={proxies.data.configRevision}
      configuration={proxies.data}
      pending={mutations.setGlobal.isPending || proxies.isFetching}
      error={mutations.setGlobal.error}
      onApply={(id) => {
        mutations.setGlobal.reset();
        mutations.setGlobal.mutate({ id, expectedRevision: proxies.data.configRevision });
      }}
    />
  );
}
