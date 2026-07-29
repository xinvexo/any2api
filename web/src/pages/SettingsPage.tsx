import { LoaderCircle, RefreshCw, Save } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Navigate, useBlocker, useParams } from "react-router-dom";

import { AboutSettings } from "@/features/application-update";
import {
  SettingsManagement,
  SETTING_SECTIONS,
  type SettingsEditor,
  useSettingsEditor,
} from "@/features/settings";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
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
  return <SettingsSectionPage section={selected} />;
}

function SettingsSectionPage({ section }: { section: (typeof SETTING_SECTIONS)[number] }) {
  const editor = useSettingsEditor(section.webGroups);
  const blocker = useBlocker(({ currentLocation, nextLocation }) =>
    editor.isDirty && locationChanged(currentLocation, nextLocation));
  const [refreshRequested, setRefreshRequested] = useState(false);
  useBeforeUnloadWarning(editor.isDirty);

  const navigationBlocked = blocker.state === "blocked";
  const dialogOpen = navigationBlocked || refreshRequested;

  async function saveAndContinue() {
    const saved = await editor.save();
    if (!saved) {
      setRefreshRequested(false);
      if (blocker.state === "blocked") {
        blocker.reset();
      }
      return;
    }
    if (refreshRequested) {
      setRefreshRequested(false);
      await editor.refresh();
      return;
    }
    if (blocker.state === "blocked") {
      blocker.proceed();
    }
  }

  function discardAndContinue() {
    editor.discard();
    if (refreshRequested) {
      setRefreshRequested(false);
      void editor.refresh();
      return;
    }
    if (blocker.state === "blocked") {
      blocker.proceed();
    }
  }

  function cancelPendingAction() {
    setRefreshRequested(false);
    if (blocker.state === "blocked") {
      blocker.reset();
    }
  }

  function refresh() {
    if (editor.isDirty) {
      setRefreshRequested(true);
      return;
    }
    void editor.refresh();
  }

  return (
    <>
      <SettingsPageLayout actions={<SettingsPageActions editor={editor} onRefresh={refresh} />}>
        <SettingsSectionBody section={section} editor={editor} />
      </SettingsPageLayout>
      <ConfirmDialog
        open={dialogOpen}
        title={refreshRequested ? "刷新前保存修改？" : "离开前保存修改？"}
        description={editor.hasValidationErrors
          ? "当前页面存在无效设置。请取消并修正，或放弃修改后继续。"
          : "当前页面有尚未保存的修改。"}
        confirmLabel={refreshRequested ? "保存并刷新" : "保存并离开"}
        cancelLabel="取消"
        alternateLabel={refreshRequested ? "放弃并刷新" : "放弃修改"}
        alternateTone="danger"
        pending={editor.isSaving}
        confirmDisabled={editor.hasValidationErrors}
        onConfirm={() => void saveAndContinue()}
        onAlternate={discardAndContinue}
        onClose={cancelPendingAction}
      />
    </>
  );
}

/**
 * Same pin pattern as system/request logs: fixed toolbar, only the body scrolls.
 * Avoids sticky chrome (and its top-offset / glass bugs) inside the main panel.
 */
function SettingsPageLayout({ children, actions }: { children: ReactNode; actions?: ReactNode }) {
  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 flex-wrap items-center gap-3 border-b border-subtle pb-3">
        <div className="min-w-0 flex-1">
          <PageTabs items={SETTINGS_TABS} ariaLabel="系统设置分类" />
        </div>
        {actions ? <div className="flex shrink-0 items-center gap-1.5">{actions}</div> : null}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto pt-5 [scrollbar-gutter:stable]">
        {children}
      </div>
    </div>
  );
}

function SettingsPageActions({ editor, onRefresh }: { editor: SettingsEditor; onRefresh: () => void }) {
  return (
    <>
      <Button
        variant="ghost"
        size="sm"
        aria-label="刷新当前设置页"
        onClick={onRefresh}
        disabled={editor.pending}
      >
        <RefreshCw size={14} className={editor.query.isFetching ? "animate-spin" : undefined} />
        <span className="hidden sm:inline">刷新</span>
      </Button>
      {editor.isDirty ? (
        <Button
          variant="primary"
          size="sm"
          onClick={() => void editor.save()}
          disabled={editor.pending || editor.hasValidationErrors}
        >
          {editor.isSaving ? <LoaderCircle size={14} className="animate-spin" /> : <Save size={14} />}
          保存
        </Button>
      ) : null}
    </>
  );
}

function SettingsSectionBody({
  section,
  editor,
}: {
  section: (typeof SETTING_SECTIONS)[number];
  editor: SettingsEditor;
}) {
  return (
    <SettingsManagement
      editor={editor}
      featuredKeys={section.featuredKeys}
      showSectionHeading={false}
    />
  );
}

function useBeforeUnloadWarning(enabled: boolean) {
  useEffect(() => {
    if (!enabled) {
      return;
    }
    const warn = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [enabled]);
}

function locationChanged(
  current: { pathname: string; search: string; hash: string },
  next: { pathname: string; search: string; hash: string },
) {
  return current.pathname !== next.pathname
    || current.search !== next.search
    || current.hash !== next.hash;
}
