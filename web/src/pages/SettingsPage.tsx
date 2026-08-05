import { LoaderCircle, RefreshCw, Save } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Navigate, useBlocker, useParams } from "react-router-dom";

import { AboutSettings } from "@/features/application-update";
import {
  SettingsManagement,
  SETTING_SECTIONS,
  type SettingsEditor,
  useSettingsEditor,
} from "@/features/settings";
import { notify } from "@/shared/notifications";
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
  const selected = SETTING_SECTIONS.find((item) => item.id === section);
  if (section !== "about" && !selected) {
    return <Navigate to="/settings/basic" replace />;
  }
  return (
    <SettingsPageLayout>
      {(actionsHost) => section === "about"
        ? <AboutSettings />
        : <SettingsSectionPage section={selected!} actionsHost={actionsHost} />}
    </SettingsPageLayout>
  );
}

function SettingsSectionPage({
  section,
  actionsHost,
}: {
  section: (typeof SETTING_SECTIONS)[number];
  actionsHost: HTMLDivElement | null;
}) {
  const editor = useSettingsEditor(section.webGroups);
  const blocker = useBlocker(({ currentLocation, nextLocation }) =>
    editor.isDirty && locationChanged(currentLocation, nextLocation));
  const [refreshRequested, setRefreshRequested] = useState(false);
  useBeforeUnloadWarning(editor.isDirty);

  const navigationBlocked = blocker.state === "blocked";
  const dialogOpen = navigationBlocked || refreshRequested;

  async function save() {
    if (await editor.save()) {
      notify.success("设置已保存");
    }
  }

  async function refreshSettings(message = "设置已刷新") {
    if (await editor.refresh()) {
      notify.success(message);
    }
  }

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
      await refreshSettings("设置已保存并刷新");
      return;
    }
    notify.success("设置已保存");
    if (blocker.state === "blocked") {
      blocker.proceed();
    }
  }

  async function discardAndContinue() {
    editor.discard();
    if (refreshRequested) {
      setRefreshRequested(false);
      await refreshSettings();
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

  function requestRefresh() {
    if (editor.isDirty) {
      setRefreshRequested(true);
      return;
    }
    void refreshSettings();
  }

  return (
    <>
      {actionsHost
        ? createPortal(
            <SettingsPageActions
              editor={editor}
              onRefresh={requestRefresh}
              onSave={() => void save()}
            />,
            actionsHost,
          )
        : null}
      <SettingsSectionBody section={section} editor={editor} onRefresh={requestRefresh} />
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
        onAlternate={() => void discardAndContinue()}
        onClose={cancelPendingAction}
      />
    </>
  );
}

/**
 * Mobile follows the document scroller; desktop pins the toolbar and scrolls the body.
 * md:-mb-4 cancels shell `p-4` bottom only for the bounded desktop workspace.
 */
function SettingsPageLayout({
  children,
}: {
  children: (actionsHost: HTMLDivElement | null) => ReactNode;
}) {
  const [actionsHost, setActionsHost] = useState<HTMLDivElement | null>(null);
  return (
    <div className="flex flex-1 flex-col md:-mb-4 md:h-full md:min-h-0 md:overflow-hidden">
      <div className="flex shrink-0 flex-wrap items-center gap-3 border-b border-subtle pb-3">
        <div className="min-w-0 flex-1">
          <PageTabs items={SETTINGS_TABS} ariaLabel="系统设置分类" />
        </div>
        <div ref={setActionsHost} className="flex shrink-0 items-center gap-1.5" />
      </div>
      <div className="management-scroll-viewport pt-5 pb-4 md:min-h-0 md:flex-1 md:overflow-y-auto md:[scrollbar-gutter:stable]">
        {children(actionsHost)}
      </div>
    </div>
  );
}

function SettingsPageActions({
  editor,
  onRefresh,
  onSave,
}: {
  editor: SettingsEditor;
  onRefresh: () => void;
  onSave: () => void;
}) {
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
          onClick={onSave}
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
  onRefresh,
}: {
  section: (typeof SETTING_SECTIONS)[number];
  editor: SettingsEditor;
  onRefresh: () => void;
}) {
  return (
    <SettingsManagement
      editor={editor}
      featuredKeys={section.featuredKeys}
      showSectionHeading={false}
      onRefresh={onRefresh}
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
