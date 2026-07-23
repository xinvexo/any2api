import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/Button";

const EXIT_DURATION_MS = 160;

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  description?: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  pending?: boolean;
  tone?: "danger" | "default";
  onConfirm: () => void;
  onClose: () => void;
}

interface DialogView {
  title: string;
  description?: ReactNode;
  confirmLabel: string;
  cancelLabel: string;
  tone: "danger" | "default";
}

export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel = "确认",
  cancelLabel = "取消",
  pending = false,
  tone = "default",
  onConfirm,
  onClose,
}: ConfirmDialogProps) {
  const titleId = useId();
  const descriptionId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  const onConfirmRef = useRef(onConfirm);

  const [view, setView] = useState<DialogView>({
    title,
    description,
    confirmLabel,
    cancelLabel,
    tone,
  });
  const [mounted, setMounted] = useState(open);
  const [visible, setVisible] = useState(false);
  const [openProp, setOpenProp] = useState(open);

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    onConfirmRef.current = onConfirm;
  }, [onConfirm]);

  if (open !== openProp) {
    setOpenProp(open);
    if (open) {
      setMounted(true);
      setView({ title, description, confirmLabel, cancelLabel, tone });
    } else {
      setVisible(false);
    }
  } else if (open) {
    const nextView = { title, description, confirmLabel, cancelLabel, tone };
    if (
      view.title !== nextView.title ||
      view.description !== nextView.description ||
      view.confirmLabel !== nextView.confirmLabel ||
      view.cancelLabel !== nextView.cancelLabel ||
      view.tone !== nextView.tone
    ) {
      setView(nextView);
    }
  }

  useEffect(() => {
    if (!open || !mounted || visible) {
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      setVisible(true);
      panelRef.current?.focus({ preventScroll: true });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [open, mounted, visible]);

  useEffect(() => {
    if (open || visible || !mounted) {
      return;
    }
    const timeout = window.setTimeout(() => setMounted(false), EXIT_DURATION_MS);
    return () => window.clearTimeout(timeout);
  }, [open, visible, mounted]);

  useEffect(() => {
    if (!mounted) {
      return;
    }

    const { body } = document;
    const previousOverflow = body.style.overflow;
    const previousPaddingRight = body.style.paddingRight;
    const scrollbarGap = window.innerWidth - document.documentElement.clientWidth;
    body.style.overflow = "hidden";
    if (scrollbarGap > 0) {
      body.style.paddingRight = `${scrollbarGap}px`;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !pending) {
        event.preventDefault();
        onCloseRef.current();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      body.style.overflow = previousOverflow;
      body.style.paddingRight = previousPaddingRight;
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [mounted, pending]);

  if (!mounted || typeof document === "undefined") {
    return null;
  }

  return createPortal(
    <div
      className="confirm-dialog-root fixed inset-0 z-[60] flex items-center justify-center overflow-hidden p-4"
      data-state={visible ? "open" : "closed"}
    >
      <button
        type="button"
        tabIndex={-1}
        className={cn("confirm-dialog-scrim", visible ? "is-open" : "is-closed")}
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
        aria-describedby={view.description ? descriptionId : undefined}
        tabIndex={-1}
        className={cn("confirm-dialog-panel", visible ? "is-open" : "is-closed")}
      >
        <div className="px-5 pt-5">
          <h2 id={titleId} className="text-[15px] font-semibold tracking-tight text-primary">
            {view.title}
          </h2>
          {view.description ? (
            <div id={descriptionId} className="mt-2 text-[13px] leading-5 text-secondary">
              {view.description}
            </div>
          ) : null}
        </div>
        <div className="mt-4 flex flex-col-reverse gap-2 px-5 pb-5 sm:flex-row sm:justify-end sm:gap-2">
          <Button
            variant="ghost"
            className="min-w-[4.5rem] sm:min-w-[5rem]"
            disabled={pending}
            onClick={() => onCloseRef.current()}
          >
            {view.cancelLabel}
          </Button>
          <Button
            variant={view.tone === "danger" ? "dangerSolid" : "primary"}
            className="min-w-[4.5rem] sm:min-w-[5rem]"
            disabled={pending}
            onClick={() => onConfirmRef.current()}
          >
            {pending ? "处理中…" : view.confirmLabel}
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
