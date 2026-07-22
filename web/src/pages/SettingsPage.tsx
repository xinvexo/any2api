import { Navigate, useParams } from "react-router-dom";

import { AdminPasswordRotation } from "@/features/admin-auth";
import { GlobalProxySettings } from "@/features/proxies";
import { SettingsManagement, SETTING_SECTIONS } from "@/features/settings";

export function SettingsPage() {
  const { section = "password" } = useParams<{ section: string }>();

  if (section === "password") {
    return <AdminPasswordRotation />;
  }

  if (section === "proxy") {
    return <GlobalProxySettings />;
  }

  const groups = SETTING_SECTIONS.find((item) => item.id === section)?.webGroups;
  if (groups) {
    return <SettingsManagement webGroups={groups} categorized={false} />;
  }

  return <Navigate to="/settings/password" replace />;
}
