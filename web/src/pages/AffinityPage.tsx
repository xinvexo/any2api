import { AffinityManagement } from "@/features/affinity";
import { SettingsManagement } from "@/features/settings";

export function AffinityPage() {
  return (
    <div className="space-y-10">
      <AffinityManagement />

      <section className="space-y-5" aria-labelledby="affinity-settings-heading">
        <header>
          <h2 id="affinity-settings-heading" className="text-lg font-semibold">
            会话策略
          </h2>
          <p className="mt-1 max-w-3xl text-sm leading-6 text-secondary">
            调整软硬粘性 TTL、prefer/strict 模式与固定 Credential 等待超时；修改后热更新到新请求。
          </p>
        </header>
        <SettingsManagement keyPrefix="affinity." />
      </section>
    </div>
  );
}
