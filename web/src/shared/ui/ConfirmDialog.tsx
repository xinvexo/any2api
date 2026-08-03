import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/Button";
import { useBodyScrollLock } from "@/shared/ui/useBodyScrollLock";

const EXIT_DURATION_MS = 160;

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  description?: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  alternateLabel?: string;
  alternateTone?: "danger" | "default";
  pending?: boolean;
  confirmDisabled?: boolean;
  tone?: "danger" | "default";
  onConfirm: () => void;
  onAlternate?: () => void;
  onClose: () => void;
}

interface DialogView {
  title: string;
  description?: string;
  confirmLabel: string;
  cancelLabel: string;
  alternateLabel?: string;
  alternateTone: "danger" | "default";
  tone: "danger" | "default";
}

export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel = "确认",
  cancelLabel = "取消",
  alternateLabel,
  alternateTone = "default",
  pending = false,
  confirmDisabled = false,
  tone = "default",
  onConfirm,
  onAlternate,
  onClose,
}: ConfirmDialogProps) {
  const titleId = useId();
  const descriptionId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  const onConfirmRef = useRef(onConfirm);
  const onAlternateRef = useRef(onAlternate);
  const closingDescription = typeof description === "string" ? description : undefined;

  const [view, setView] = useState<DialogView>({
    title,
    description: closingDescription,
    confirmLabel,
    cancelLabel,
    alternateLabel,
    alternateTone,
    tone,
  });
  const [mounted, setMounted] = useState(open);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    onConfirmRef.current = onConfirm;
  }, [onConfirm]);

  useEffect(() => {
    onAlternateRef.current = onAlternate;
  }, [onAlternate]);

  useEffect(() => {
    if (!open) {
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      setView({
        title,
        description: closingDescription,
        confirmLabel,
        cancelLabel,
        alternateLabel,
        alternateTone,
        tone,
      });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [
    open,
    title,
    closingDescription,
    confirmLabel,
    cancelLabel,
    alternateLabel,
    alternateTone,
    tone,
  ]);

  useEffect(() => {
    if (open && !mounted) {
      const frame = window.requestAnimationFrame(() => setMounted(true));
      return () => window.cancelAnimationFrame(frame);
    }

    if (open && mounted && !visible) {
      const frame = window.requestAnimationFrame(() => {
        setVisible(true);
        panelRef.current?.focus({ preventScroll: true });
      });
      return () => window.cancelAnimationFrame(frame);
    }

    if (!open && mounted) {
      const timeout = window.setTimeout(() => {
        setVisible(false);
        setMounted(false);
      }, EXIT_DURATION_MS);
      return () => window.clearTimeout(timeout);
    }
  }, [open, mounted, visible]);

  useEffect(() => {
    if (!mounted) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !pending) {
        event.preventDefault();
        onCloseRef.current();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [mounted, pending]);

  useBodyScrollLock(mounted);

  if (!mounted || typeof document === "undefined") {
    return null;
  }

  const activeView = open
    ? {
        title,
        confirmLabel,
        cancelLabel,
        alternateLabel,
        alternateTone,
        tone,
      }
    : view;
  const activeDescription = open ? description : view.description;
  const isVisible = open && visible;

  return createPortal(
    <div
      className="confirm-dialog-root fixed inset-0 z-[60] flex items-center justify-center overflow-hidden p-4"
      data-state={isVisible ? "open" : "closed"}
    >
      <button
        type="button"
        tabIndex={-1}
        className={cn("confirm-dialog-scrim", isVisible ? "is-open" : "is-closed")}
        aria-label="关闭对话框"
        disabled={pending}
        onClick={() => {
          if (!pending) {
            onCloseRef.current();
          }
        }}
      />
      <div
        ref={panelRef}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={activeDescription ? descriptionId : undefined}
        tabIndex={-1}
        className={cn("confirm-dialog-panel", isVisible ? "is-open" : "is-closed")}
      >
        <div className="px-5 pt-5">
          <h2 id={titleId} className="text-[15px] font-semibold tracking-tight text-primary">
            {activeView.title}
          </h2>
          {activeDescription ? (
            <div id={descriptionId} className="mt-2 text-[13px] leading-5 text-secondary">
              {activeDescription}
            </div>
          ) : null}
        </div>
        <div className="mt-5 flex items-center justify-end gap-2 border-t border-subtle/70 px-5 py-4">
          <Button
            variant="secondary"
            className="min-w-[4.5rem]"
            disabled={pending}
            onClick={() => onCloseRef.current()}
          >
            {activeView.cancelLabel}
          </Button>
          {activeView.alternateLabel && onAlternate ? (
            <Button
              variant={activeView.alternateTone === "danger" ? "dangerSolid" : "secondary"}
              className="min-w-[4.5rem]"
              disabled={pending}
              onClick={() => onAlternateRef.current?.()}
            >
              {activeView.alternateLabel}
            </Button>
          ) : null}
          <Button
            variant={activeView.tone === "danger" ? "dangerSolid" : "primary"}
            className="min-w-[4.5rem]"
            disabled={pending || confirmDisabled}
            onClick={() => onConfirmRef.current()}
          >
            {pending ? "处理中…" : activeView.confirmLabel}
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
