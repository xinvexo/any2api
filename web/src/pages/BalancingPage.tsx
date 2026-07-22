import { BalancingManagement } from "@/features/balancing";
import { SettingsManagement } from "@/features/settings";

export function BalancingPage() {
  return (
    <div className="space-y-10">
      <section className="space-y-7">
        <header>
          <p className="text-sm font-medium text-accent-copy">运行态调度</p>
          <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">负载均衡</h1>
          <p className="mt-3 max-w-3xl text-sm leading-6 text-secondary">按 Credential 最大并发与当前占用率选择上游；容量、排队和健康状态只存在当前进程内。</p>
        </header>
        <BalancingManagement />
      </section>
      <section className="space-y-5" aria-labelledby="balancing-settings-heading">
        <header>
          <p className="text-sm font-medium text-accent-copy">默认值与覆盖值</p>
          <h2 id="balancing-settings-heading" className="mt-2 text-2xl font-semibold">调度策略</h2>
          <p className="mt-2 max-w-3xl text-sm leading-6 text-secondary">调整满载行为、队列上限、等待超时、fallback tier 和辅助请求并发；修改只影响新请求。</p>
        </header>
        <SettingsManagement keyPrefix="scheduler." />
      </section>
    </div>
  );
}
