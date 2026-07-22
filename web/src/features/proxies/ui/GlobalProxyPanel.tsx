import { Globe2 } from "lucide-react";
import { useState } from "react";

import type { ProxyConfiguration } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

interface GlobalProxyPanelProps {
  configuration: ProxyConfiguration;
  pending: boolean;
  error: unknown;
  onApply: (id: string) => void;
}

export function GlobalProxyPanel({
  configuration,
  pending,
  error,
  onApply,
}: GlobalProxyPanelProps) {
  const [selected, setSelected] = useState(configuration.globalProxyId);
  const current = configuration.items.find((item) => item.id === configuration.globalProxyId);

  return (
    <Surface className="overflow-hidden">
      <div className="grid gap-6 p-5 sm:p-6 lg:grid-cols-[minmax(0,1fr)_minmax(280px,0.55fr)] lg:items-end">
        <div className="flex gap-4">
          <span className="grid size-11 shrink-0 place-items-center rounded-control bg-surface-muted text-accent-copy">
            <Globe2 size={20} aria-hidden="true" />
          </span>
          <div className="min-w-0">
            <p className="text-sm font-semibold">全局代理</p>
            <p className="mt-1 break-words text-lg font-semibold [overflow-wrap:anywhere]">
              {current?.name ?? "未知代理"}
            </p>
            <p className="mt-2 max-w-2xl text-sm leading-6 text-secondary">
              Credential 绑定 DIRECT 时会继承此出口；这里也是 DIRECT 时，最终从本机直连。专属代理失败不会回退到全局代理。
            </p>
          </div>
        </div>
        <div>
          <label htmlFor="global-proxy" className="text-sm font-medium">
            选择全局出口
          </label>
          <div className="mt-2 flex flex-col gap-2 sm:flex-row">
            <select
              id="global-proxy"
              className="focus-ring h-8 min-w-0 flex-1 rounded-control border border-subtle bg-surface px-2.5 text-[12px]"
              value={selected}
              onChange={(event) => setSelected(event.target.value)}
              disabled={pending}
            >
              {configuration.items
                .filter((proxy) => proxy.enabled)
                .map((proxy) => (
                  <option key={proxy.id} value={proxy.id}>
                    {proxy.name} · {proxy.kind.toUpperCase()}
                  </option>
                ))}
            </select>
            <Button
              variant="primary"
              disabled={pending || selected === configuration.globalProxyId}
              onClick={() => onApply(selected)}
            >
              应用
            </Button>
          </div>
          {error ? (
            <p className="mt-2 text-sm text-danger" role="alert">
              {getProxyErrorMessage(error)}
            </p>
          ) : null}
        </div>
      </div>
    </Surface>
  );
}
