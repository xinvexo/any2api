import { AffinityManagement } from "@/features/affinity";
import { SettingsManagement } from "@/features/settings";

export function AffinityPage() {
  return (
    <div className="space-y-10">
      <section className="space-y-7">
        <header>
          <p className="text-sm font-medium text-accent-copy">运行态路由</p>
          <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">会话粘性</h1>
          <p className="mt-3 max-w-3xl text-sm leading-6 text-secondary">
            普通会话使用可配置的软粘性；Codex previous_response_id 使用精确硬绑定。所有绑定只存在内存中，程序重启后自动清空。
          </p>
        </header>
        <AffinityManagement />
      </section>

      <section className="space-y-5" aria-labelledby="affinity-settings-heading">
        <header>
          <p className="text-sm font-medium text-accent-copy">默认值与覆盖值</p>
          <h2 id="affinity-settings-heading" className="mt-2 text-2xl font-semibold">
            会话策略
          </h2>
          <p className="mt-2 max-w-3xl text-sm leading-6 text-secondary">
            调整软硬粘性 TTL、prefer/strict 模式与固定 Credential 等待超时；修改后热更新到新请求。
          </p>
        </header>
        <SettingsManagement keyPrefix="affinity." />
      </section>
    </div>
  );
}
