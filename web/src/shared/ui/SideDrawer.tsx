import { X } from "lucide-react";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { cn } from "@/shared/lib/cn";
import { IconButton } from "@/shared/ui/IconButton";
import { useBodyScrollLock } from "@/shared/ui/useBodyScrollLock";
import { useModalFocus } from "@/shared/ui/useModalFocus";

/** Keep in sync with `.side-drawer-panel` / `.side-drawer-scrim` transition duration. */
const EXIT_DURATION_MS = 200;

interface SideDrawerProps {
  open: boolean;
  title: string;
  description?: string;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
}

interface DrawerView {
  title: string;
  description?: string;
  wide: boolean;
}

export function SideDrawer({
  open,
  title,
  description,
  onClose,
  children,
  wide = false,
}: SideDrawerProps) {
  const titleId = useId();
  const descriptionId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);

  const [view, setView] = useState<DrawerView>({ title, description, wide });
  const [mounted, setMounted] = useState(open);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    if (!open) {
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      setView({ title, description, wide });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [open, title, description, wide]);

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

  useBodyScrollLock(mounted);
  useModalFocus(rootRef, mounted, () => onCloseRef.current());

  if (!mounted || typeof document === "undefined") {
    return null;
  }

  const activeView = open ? { title, description, wide } : view;
  const isVisible = open && visible;

  return createPortal(
    <div
      ref={rootRef}
      className="side-drawer-root fixed inset-0 z-50 overflow-hidden"
      data-state={isVisible ? "open" : "closed"}
      data-overlay-root
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={activeView.description ? descriptionId : undefined}
    >
      <button
        type="button"
        tabIndex={-1}
        className={cn("side-drawer-scrim", isVisible ? "is-open" : "is-closed")}
        aria-label="关闭抽屉"
        onClick={() => onCloseRef.current()}
      />
      <div
        ref={panelRef}
        tabIndex={-1}
        className={cn(
          "side-drawer-panel",
          activeView.wide ? "is-wide" : undefined,
          isVisible ? "is-open" : "is-closed",
        )}
      >
        <header className="flex shrink-0 items-start justify-between gap-3 border-b border-subtle/70 px-5 py-4">
          <div className="min-w-0">
            <h2 id={titleId} className="text-[15px] font-semibold tracking-tight">
              {activeView.title}
            </h2>
            {activeView.description ? (
              <p id={descriptionId} className="mt-1 text-[13px] leading-5 text-secondary">
                {activeView.description}
              </p>
            ) : null}
          </div>
          <IconButton
            label="关闭"
            className="shrink-0"
            onClick={() => onCloseRef.current()}
          >
            <X size={16} strokeWidth={1.75} />
          </IconButton>
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5 [scrollbar-gutter:stable]">
          {open ? children : null}
        </div>
      </div>
    </div>,
    document.body,
  );
}
