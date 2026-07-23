import { Navigate, useParams } from "react-router-dom";

import { AdminPasswordRotation } from "@/features/admin-auth";
import { GlobalProxySettings } from "@/features/proxies";
import { SettingsManagement, SETTING_SECTIONS } from "@/features/settings";
import { PageTabs } from "@/shared/ui/PageTabs";

const SETTINGS_TABS = [
  { label: "管理员密码", path: "/settings/password" },
  { label: "基础设置", path: "/settings/basic" },
  ...SETTING_SECTIONS.filter((section) => section.id !== "admin").map((section) => ({
    label: section.label,
    path: `/settings/${section.id}`,
  })),
] as const;

export function SettingsPage() {
  const { section = "password" } = useParams<{ section: string }>();

  const known =
    section === "password" ||
    section === "basic" ||
    SETTING_SECTIONS.some((item) => item.id === section && item.id !== "admin");

  if (section === "proxy" || section === "admin") {
    return <Navigate to="/settings/basic" replace />;
  }

  if (!known) {
    return <Navigate to="/settings/password" replace />;
  }

  return (
    <div className="space-y-5">
      <div className="border-b border-subtle pb-2">
        <PageTabs items={SETTINGS_TABS} ariaLabel="系统设置分类" />
      </div>
      <SettingsSectionBody section={section} />
    </div>
  );
}

function SettingsSectionBody({ section }: { section: string }) {
  if (section === "password") {
    return <AdminPasswordRotation />;
  }

  if (section === "basic") {
    return (
      <div className="space-y-8">
        <GlobalProxySettings />
        <SettingsManagement webGroups={["远程管理"]} showSectionHeading={false} />
      </div>
    );
  }

  const groups = SETTING_SECTIONS.find((item) => item.id === section)?.webGroups;
  if (groups) {
    return <SettingsManagement webGroups={groups} showSectionHeading={false} />;
  }

  return <Navigate to="/settings/password" replace />;
}
