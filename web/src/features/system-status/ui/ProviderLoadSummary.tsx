import { useBalancingRuntime } from "../model/use-balancing-runtime";
import { providerKindLabel } from "@/shared/api/provider-protocol-vocabulary";

export function ProviderLoadSummary() {
  const runtime = useBalancingRuntime().data;
  if (!runtime?.providers.length) {
    return null;
  }

  return (
    <section className="min-w-0 border-t border-subtle pt-5" aria-labelledby="provider-load-title">
      <div className="flex items-baseline justify-between gap-4">
        <h2 id="provider-load-title" className="text-sm font-semibold tracking-tight">
          Provider 负载
        </h2>
        <p className="text-[11px] tabular-nums text-tertiary">近 60 秒</p>
      </div>
      <ul className="mt-3 grid min-w-0 gap-2 sm:grid-cols-2">
        {runtime.providers.map((provider) => (
          <li
            key={provider.providerKind}
            className="flex min-w-0 items-center justify-between gap-4 rounded-[14px] bg-surface-muted/45 px-3.5 py-3"
          >
            <span className="truncate text-sm font-medium">
              {providerKindLabel(provider.providerKind)}
            </span>
            <span className="shrink-0 text-right text-[11px] tabular-nums text-secondary">
              {formatCount(provider.requestsInWindow)} 次 · {formatCount(provider.inFlight)} 进行中
              {provider.limitedCredentialCount > 0
                ? ` · ${formatCount(provider.rateLimitedCredentialCount)}/${formatCount(provider.limitedCredentialCount)} 达到每分钟上限`
                : ""}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}

function formatCount(value: number) {
  return value.toLocaleString("zh-CN");
}
