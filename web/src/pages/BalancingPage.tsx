import { BalancingManagement } from "@/features/balancing";
import { SettingsManagement } from "@/features/settings";

export function BalancingPage() {
  return (
    <div className="space-y-10">
      <BalancingManagement />

      <section className="space-y-5" aria-labelledby="balancing-settings-heading">
        <header>
          <h2 id="balancing-settings-heading" className="text-lg font-semibold">
            调度策略
          </h2>
          <p className="mt-1 max-w-3xl text-sm leading-6 text-secondary">
            调整满载行为、队列上限、等待超时、fallback tier 和辅助请求并发；修改只影响新请求。
          </p>
        </header>
        <SettingsManagement keyPrefix="scheduler." />
      </section>
    </div>
  );
}
