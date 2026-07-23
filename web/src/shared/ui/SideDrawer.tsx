import { X } from "lucide-react";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { cn } from "@/shared/lib/cn";
import { IconButton } from "@/shared/ui/IconButton";

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
  children: ReactNode;
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
  const panelRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  const openRef = useRef(open);

  const [view, setView] = useState<DrawerView>({ title, description, children, wide });
  const [mounted, setMounted] = useState(open);
  const [visible, setVisible] = useState(false);
  const [openProp, setOpenProp] = useState(open);

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  openRef.current = open;

  // Adjust local state when the controlled `open` prop changes (React-supported pattern).
  // While closing (`open === false` but still mounted), freeze the last open view so parent
  // unmounts of form state cannot flash empty content through the exit animation.
  if (open !== openProp) {
    setOpenProp(open);
    if (open) {
      setMounted(true);
      setView({ title, description, children, wide });
    } else {
      // Drop children immediately so secret fields never linger through exit animation.
      setView((current) => ({ ...current, children: null }));
      setVisible(false);
    }
  } else if (open) {
    const nextView = { title, description, children, wide };
    if (
      view.title !== nextView.title ||
      view.description !== nextView.description ||
      view.children !== nextView.children ||
      view.wide !== nextView.wide
    ) {
      setView(nextView);
    }
  }

  useEffect(() => {
    if (!open || !mounted || visible) {
      return;
    }

    const frame = window.requestAnimationFrame(() => {
      if (!openRef.current) {
        return;
      }
      setVisible(true);
      panelRef.current?.focus({ preventScroll: true });
    });

    return () => window.cancelAnimationFrame(frame);
  }, [open, mounted, visible]);

  useEffect(() => {
    if (open || visible || !mounted) {
      return;
    }

    const timeout = window.setTimeout(() => {
      setMounted(false);
    }, EXIT_DURATION_MS);
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
      if (event.key === "Escape") {
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
  }, [mounted]);

  if (!mounted || typeof document === "undefined") {
    return null;
  }

  return createPortal(
    <div
      className="side-drawer-root fixed inset-0 z-50 overflow-hidden"
      data-state={visible ? "open" : "closed"}
    >
      <button
        type="button"
        tabIndex={-1}
        className={cn("side-drawer-scrim", visible ? "is-open" : "is-closed")}
        aria-label="关闭抽屉"
        onClick={() => onCloseRef.current()}
      />
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={view.description ? descriptionId : undefined}
        tabIndex={-1}
        className={cn(
          "side-drawer-panel",
          view.wide ? "is-wide" : undefined,
          visible ? "is-open" : "is-closed",
        )}
      >
        <header className="flex shrink-0 items-start justify-between gap-3 border-b border-subtle px-5 py-4">
          <div className="min-w-0">
            <h2 id={titleId} className="text-[15px] font-semibold tracking-tight">
              {view.title}
            </h2>
            {view.description ? (
              <p id={descriptionId} className="mt-1 text-[13px] leading-5 text-secondary">
                {view.description}
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
        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5">{view.children}</div>
      </div>
    </div>,
    document.body,
  );
}
