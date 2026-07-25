import { Activity, Gauge, ListChecks, ShieldAlert } from "lucide-react";

import type { BalancingRuntime } from "../api/balancing-contracts";
import { Surface } from "@/shared/ui/Surface";

export function BalancingSummary({ runtime }: { runtime: BalancingRuntime }) {
  return (
    <div className="space-y-4">
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Metric icon={Gauge} label="RPM 窗口已用" value={String(runtime.totals.requestsInWindow)} detail={`${runtime.totals.limitedCredentialCount} 个 Credential 启用 RPM`} />
        <Metric icon={ShieldAlert} label="RPM 已用尽" value={`${runtime.totals.rateLimitedCredentialCount} / ${runtime.totals.limitedCredentialCount}`} detail="精确滚动 60 秒窗口" />
        <Metric icon={Activity} label="处理中" value={String(runtime.totals.inFlight)} detail="仅用于观测，不参与准入或排序" />
        <Metric icon={ListChecks} label="排队" value={`${runtime.queue.waiting} / ${runtime.queue.maxWaiting}`} detail={runtime.queue.onRateLimited === "wait" ? `最多等待 ${formatDuration(runtime.queue.timeoutSecs)} · 固定等待 ${runtime.totals.fixedWaiters}` : "RPM 用尽时立即拒绝"} />
      </div>
      {runtime.providers.length > 0 ? (
        <Surface className="grid gap-px overflow-hidden bg-subtle sm:grid-cols-2">
          {runtime.providers.map((provider) => (
            <div key={provider.providerKind} className="bg-surface px-5 py-4">
              <div className="flex items-center justify-between gap-4">
                <p className="font-semibold">{provider.providerKind === "codex" ? "Codex" : "Claude"}</p>
                <p className="text-sm tabular-nums text-secondary">{provider.requestsInWindow} 次 / 60 秒</p>
              </div>
              <p className="mt-2 text-xs text-tertiary">{provider.limitedCredentialCount} / {provider.credentialCount} 启用 RPM · {provider.rateLimitedCredentialCount} 个已用尽 · 选中 {provider.selected}</p>
            </div>
          ))}
        </Surface>
      ) : null}
    </div>
  );
}

function Metric({ icon: Icon, label, value, detail }: { icon: typeof Activity; label: string; value: string; detail: string }) {
  return (
    <Surface className="p-5">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-secondary">{label}</p>
        <Icon size={17} className="text-tertiary" aria-hidden="true" />
      </div>
      <p className="mt-3 text-2xl font-semibold tabular-nums">{value}</p>
      <p className="mt-2 text-xs leading-5 text-tertiary">{detail}</p>
    </Surface>
  );
}

function formatDuration(seconds: number) {
  return `${seconds} 秒`;
}
