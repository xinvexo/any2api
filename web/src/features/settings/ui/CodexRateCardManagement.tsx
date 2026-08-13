import { LoaderCircle, RefreshCw, Save } from "lucide-react";
import { useEffect, useState } from "react";
import { useBlocker } from "react-router-dom";

import { getSettingsErrorMessage } from "../model/settings-error";
import { useCodexRateCardEditor } from "../model/use-codex-rate-card-editor";
import { CodexRateCardForm } from "./CodexRateCardForm";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { Surface } from "@/shared/ui/Surface";

export function CodexRateCardManagement() {
  const editor = useCodexRateCardEditor();
  const blocker = useBlocker(({ currentLocation, nextLocation }) =>
    editor.isDirty && locationChanged(currentLocation, nextLocation));
  const [refreshRequested, setRefreshRequested] = useState(false);
  useBeforeUnloadWarning(editor.isDirty);

  async function save() {
    if (await editor.save()) notify.success("额度费率已保存");
  }

  async function refresh(message = "额度费率已刷新") {
    if (await editor.refresh()) notify.success(message);
  }

  async function saveAndContinue() {
    if (!await editor.save()) {
      setRefreshRequested(false);
      if (blocker.state === "blocked") blocker.reset();
      return;
    }
    if (refreshRequested) {
      setRefreshRequested(false);
      await refresh("额度费率已保存并刷新");
    } else {
      notify.success("额度费率已保存");
      if (blocker.state === "blocked") blocker.proceed();
    }
  }

  async function discardAndContinue() {
    editor.discard();
    if (refreshRequested) {
      setRefreshRequested(false);
      await refresh();
    } else if (blocker.state === "blocked") {
      blocker.proceed();
    }
  }

  function requestRefresh() {
    if (editor.isDirty) setRefreshRequested(true);
    else void refresh();
  }

  const dialogOpen = refreshRequested || blocker.state === "blocked";
  return (
    <div className="flex h-full min-h-0 flex-col" aria-busy={editor.pending}>
      <header className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-subtle pb-3">
        <h1 className="text-[16px] font-semibold tracking-tight">Codex 额度费率</h1>
        <div className="flex items-center gap-1.5">
          <Button
            variant="ghost"
            size="sm"
            aria-label="刷新额度费率"
            disabled={editor.pending}
            onClick={requestRefresh}
          >
            <RefreshCw size={14} className={editor.query.isFetching ? "animate-spin" : undefined} />
            <span className="hidden sm:inline">刷新</span>
          </Button>
          {editor.isDirty ? (
            <Button
              variant="primary"
              size="sm"
              disabled={editor.pending || editor.hasValidationErrors}
              onClick={() => void save()}
            >
              {editor.isSaving ? <LoaderCircle size={14} className="animate-spin" /> : <Save size={14} />}
              保存
            </Button>
          ) : null}
        </div>
      </header>

      <div className="management-scroll-viewport min-h-0 flex-1 overflow-y-auto pb-4 pt-2 md:[scrollbar-gutter:stable]">
        <CodexRateCardBody editor={editor} onRefresh={requestRefresh} />
      </div>

      <ConfirmDialog
        open={dialogOpen}
        title={refreshRequested ? "刷新前保存修改？" : "离开前保存修改？"}
        description={editor.hasValidationErrors
          ? "当前页面存在无效费率。请取消并修正，或放弃修改后继续。"
          : "当前页面有尚未保存的修改。"}
        confirmLabel={refreshRequested ? "保存并刷新" : "保存并离开"}
        alternateLabel={refreshRequested ? "放弃并刷新" : "放弃修改"}
        alternateTone="danger"
        pending={editor.isSaving}
        confirmDisabled={editor.hasValidationErrors}
        onConfirm={() => void saveAndContinue()}
        onAlternate={() => void discardAndContinue()}
        onClose={() => {
          setRefreshRequested(false);
          if (blocker.state === "blocked") blocker.reset();
        }}
      />
    </div>
  );
}

function CodexRateCardBody({
  editor,
  onRefresh,
}: {
  editor: ReturnType<typeof useCodexRateCardEditor>;
  onRefresh: () => void;
}) {
  if (editor.query.isPending && !editor.query.data) {
    return <p className="py-20 text-center text-sm text-secondary" aria-busy="true">正在读取额度费率</p>;
  }
  if (!editor.query.data || !editor.item || !editor.card || !editor.draft || !editor.validation) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取额度费率</p>
        <p className="mt-2 text-sm text-secondary">{getSettingsErrorMessage(editor.query.error)}</p>
        <Button className="mt-5" onClick={onRefresh} disabled={editor.query.isFetching}>重试</Button>
      </Surface>
    );
  }
  return (
    <>
      {editor.query.isError ? (
        <Surface className="mb-4 border-warning/40 p-4 text-sm text-secondary" role="status">
          刷新失败，当前仍显示最近一次有效数据：{getSettingsErrorMessage(editor.query.error)}
        </Surface>
      ) : null}
      {editor.saveError ? (
        <Surface className="mb-4 border-danger/40 p-4 text-sm text-secondary" role="alert">
          保存失败，修改仍保留：{getSettingsErrorMessage(editor.saveError)}
        </Surface>
      ) : null}
      <CodexRateCardForm
        item={editor.item}
        value={editor.draft}
        errors={editor.validation.errors}
        disabled={editor.pending}
        onChange={editor.setDraft}
      />
    </>
  );
}

function useBeforeUnloadWarning(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
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
