import { AdminPasswordRotation } from "@/features/admin-auth";
import { GlobalProxySettings } from "@/features/proxies";
import { SettingsManagement } from "@/features/settings";

export function SettingsPage() {
  return (
    <div className="space-y-7">
      <AdminPasswordRotation />
      <GlobalProxySettings />
      <SettingsManagement />
    </div>
  );
}
