import { useState } from "react";

import type { ProxyConfiguration } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { Button } from "@/shared/ui/Button";

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
    <section aria-labelledby="global-proxy-heading">
      <header className="mb-4">
        <h2 id="global-proxy-heading" className="text-[15px] font-semibold tracking-tight">
          全局代理
        </h2>
        <p className="mt-1 max-w-2xl text-[12px] leading-5 text-secondary">
          Credential 绑定 DIRECT 时会继承此出口；这里也是 DIRECT 时，最终从本机直连。专属代理失败不会回退到全局代理。
        </p>
      </header>

      <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
        <div className="min-w-0">
          <label htmlFor="global-proxy" className="text-[12px] font-medium">
            当前出口 · {current?.name ?? "未知代理"}
          </label>
          <select
            id="global-proxy"
            className="field-select focus-ring mt-1.5 h-8 w-full min-w-0 cursor-pointer appearance-none rounded-[8px] border-0 bg-surface-muted py-0 pl-2.5 pr-8 text-[12px]"
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
        </div>
        <Button
          variant="primary"
          disabled={pending || selected === configuration.globalProxyId}
          onClick={() => onApply(selected)}
        >
          应用
        </Button>
      </div>

      {error ? (
        <p className="mt-3 text-[12px] text-danger" role="alert">
          {getProxyErrorMessage(error)}
        </p>
      ) : null}
    </section>
  );
}
