import { Activity, Clock3, Layers3, ListChecks } from "lucide-react";

import type { BalancingRuntime } from "../api/balancing-contracts";
import { Surface } from "@/shared/ui/Surface";

export function BalancingSummary({ runtime }: { runtime: BalancingRuntime }) {
  const load = runtime.totals.maxConcurrency === 0
    ? 0
    : Math.round((runtime.totals.inFlight / runtime.totals.maxConcurrency) * 100);
  return (
    <div className="space-y-4">
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Metric icon={Activity} label="生成并发" value={`${runtime.totals.inFlight} / ${runtime.totals.maxConcurrency}`} detail={`${load}% 当前占用`} />
        <Metric icon={ListChecks} label="排队" value={`${runtime.queue.waiting} / ${runtime.queue.maxWaiting}`} detail={runtime.queue.onSaturated === "wait" ? `最多等待 ${formatDuration(runtime.queue.timeoutMs)}` : "满载时立即拒绝"} />
        <Metric icon={Clock3} label="固定等待" value={String(runtime.totals.fixedWaiters)} detail="硬粘性与 strict/prefer 固定目标" />
        <Metric icon={Layers3} label="辅助并发" value={`${runtime.auxiliary.inFlight} / ${runtime.auxiliary.maxGlobal}`} detail={`每 Credential 上限 ${runtime.auxiliary.maxPerCredential}`} />
      </div>
      {runtime.providers.length > 0 ? (
        <Surface className="grid gap-px overflow-hidden bg-subtle sm:grid-cols-2">
          {runtime.providers.map((provider) => (
            <div key={provider.providerKind} className="bg-surface px-5 py-4">
              <div className="flex items-center justify-between gap-4">
                <p className="font-semibold">{provider.providerKind === "codex" ? "Codex" : "Claude"}</p>
                <p className="text-sm tabular-nums text-secondary">{provider.inFlight} / {provider.maxConcurrency}</p>
              </div>
              <p className="mt-2 text-xs text-tertiary">{provider.credentialCount} 个 Credential · 生成选中 {provider.selectedGeneration} · 辅助选中 {provider.selectedAuxiliary}</p>
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

function formatDuration(milliseconds: number) {
  return milliseconds >= 1_000 ? `${milliseconds / 1_000} 秒` : `${milliseconds} ms`;
}
