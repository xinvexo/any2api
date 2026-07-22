import { AdminPasswordRotation } from "@/features/admin-auth";
import { SettingsManagement } from "@/features/settings";

export function SettingsPage() {
  return (
    <div className="space-y-7">
      <header>
        <p className="text-sm font-medium text-accent-copy">运行参数</p>
        <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">设置</h1>
        <p className="mt-3 max-w-2xl text-sm leading-6 text-secondary">集中管理管理员凭据和运行参数。设置项同时展示编译默认值、用户覆盖值与当前生效值。</p>
      </header>
      <AdminPasswordRotation />
      <SettingsManagement />
    </div>
  );
}
