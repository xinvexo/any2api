import type { ReactNode } from "react";
import { Navigate, useParams } from "react-router-dom";

import { AdminPasswordRotation } from "@/features/admin-auth";
import { AboutSettings } from "@/features/application-update";
import { SettingsManagement, SETTING_SECTIONS } from "@/features/settings";
import { PageTabs } from "@/shared/ui/PageTabs";

const SETTINGS_TABS = [
  ...SETTING_SECTIONS.map((section) => ({
    label: section.label,
    path: `/settings/${section.id}`,
  })),
  { label: "关于", path: "/settings/about" },
];

export function SettingsPage() {
  const { section = "basic" } = useParams<{ section: string }>();
  if (section === "about") {
    return <SettingsPageLayout><AboutSettings /></SettingsPageLayout>;
  }
  const selected = SETTING_SECTIONS.find((item) => item.id === section);
  if (!selected) {
    return <Navigate to="/settings/basic" replace />;
  }

  return <SettingsPageLayout><SettingsSectionBody section={selected} /></SettingsPageLayout>;
}

function SettingsPageLayout({ children }: { children: ReactNode }) {
  return (
    <div className="space-y-5">
      <div className="border-b border-subtle pb-2">
        <PageTabs items={SETTINGS_TABS} ariaLabel="系统设置分类" />
      </div>
      {children}
    </div>
  );
}

function SettingsSectionBody({ section }: { section: (typeof SETTING_SECTIONS)[number] }) {
  if (section.id === "basic") {
    return (
      <div className="space-y-8">
        <AdminPasswordRotation />
        <SettingsManagement
          webGroups={section.webGroups}
          featuredKeys={section.featuredKeys}
          showSectionHeading={false}
        />
      </div>
    );
  }

  return (
    <SettingsManagement
      webGroups={section.webGroups}
      featuredKeys={section.featuredKeys}
      showSectionHeading={false}
    />
  );
}
