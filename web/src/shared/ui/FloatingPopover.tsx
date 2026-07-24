import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { cn } from "@/shared/lib/cn";

/** Viewport point the caret should aim at. */
export interface FloatingPopoverAnchor {
  x: number;
  y: number;
}

export interface FloatingPopoverProps {
  open: boolean;
  /** Caret target in viewport coordinates (usually element center-top). */
  anchor: FloatingPopoverAnchor | null;
  /**
   * Optional clamp rectangle (e.g. card bounds).
   * When omitted, the bubble stays within the viewport.
   */
  bounds?: DOMRect | null;
  /** Preferred side of the anchor. Default `auto` (above, flip below if needed). */
  placement?: "auto" | "above" | "below";
  gap?: number;
  id?: string;
  role?: "tooltip" | "dialog";
  className?: string;
  children: ReactNode;
}

interface Layout {
  left: number;
  top: number;
  caretX: number;
  showBelow: boolean;
}

const EDGE_PAD = 8;
const DEFAULT_GAP = 10;
const CARET_INSET = 14;
/** Half-diagonal of the rotated square caret (visual tip length ~5–6px). */
const CARET_HALF = 5;

/**
 * Shared macOS-style floating popover for hover/focus hints.
 * Cloud-like bubble: one surface + seamless rotated caret (drop-shadow wraps both).
 */
export function FloatingPopover({
  open,
  anchor,
  bounds = null,
  placement = "auto",
  gap = DEFAULT_GAP,
  id,
  role = "tooltip",
  className,
  children,
}: FloatingPopoverProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const [layout, setLayout] = useState<Layout | null>(null);

  useLayoutEffect(() => {
    if (!open || !anchor || !panelRef.current) {
      setLayout(null);
      return;
    }

    const tip = panelRef.current.getBoundingClientRect();
    const clamp = bounds ?? viewportBounds();
    const minLeft = clamp.left + EDGE_PAD;
    const maxLeft = Math.max(minLeft, clamp.right - EDGE_PAD - tip.width);
    const left = Math.min(Math.max(anchor.x - tip.width / 2, minLeft), maxLeft);

    const aboveTop = anchor.y - gap - tip.height;
    const belowTop = anchor.y + gap;
    const minTop = clamp.top + EDGE_PAD;
    const maxTop = Math.max(minTop, clamp.bottom - EDGE_PAD - tip.height);

    let top: number;
    let showBelow: boolean;
    if (placement === "below") {
      top = Math.min(Math.max(belowTop, minTop), maxTop);
      showBelow = true;
    } else if (placement === "above") {
      top = Math.min(Math.max(aboveTop, minTop), maxTop);
      showBelow = false;
    } else if (aboveTop < minTop) {
      top = Math.min(belowTop, maxTop);
      showBelow = true;
    } else {
      top = Math.min(Math.max(aboveTop, minTop), maxTop);
      showBelow = false;
    }

    const caretX = Math.min(
      Math.max(anchor.x - left, CARET_INSET),
      tip.width - CARET_INSET,
    );
    setLayout({ left, top, caretX, showBelow });
  }, [open, anchor, bounds, placement, gap, children]);

  if (!open || !anchor || typeof document === "undefined") {
    return null;
  }

  const showBelow = layout?.showBelow ?? false;
  const caretX = layout?.caretX;

  return createPortal(
    <div
      ref={panelRef}
      id={id}
      role={role}
      className="floating-popover pointer-events-none fixed z-[80]"
      style={{
        left: layout?.left ?? anchor.x,
        top: layout?.top ?? Math.max(anchor.y - gap, EDGE_PAD),
        visibility: layout ? "visible" : "hidden",
      }}
    >
      <div
        className={cn(
          "floating-popover__cloud relative rounded-[11px] border border-subtle bg-surface",
          "px-2.5 py-1.5 text-[11px] leading-4 text-primary",
          className,
        )}
      >
        {children}
        {/*
          Seamless speech-bubble tip: a surface square rotated 45° sits on the
          edge so fill + border read as one cloud shape (not a detached triangle).
        */}
        <span
          aria-hidden="true"
          className={cn(
            "floating-popover__tip absolute size-[10px] rotate-45 bg-surface",
            showBelow
              ? "border-l border-t border-subtle"
              : "border-r border-b border-subtle",
          )}
          style={{
            left: caretX ?? "50%",
            ...(showBelow
              ? { top: -CARET_HALF, transform: "translateX(-50%) rotate(45deg)" }
              : {
                  bottom: -CARET_HALF,
                  transform: "translateX(-50%) rotate(45deg)",
                }),
          }}
        />
      </div>
    </div>,
    document.body,
  );
}

/**
 * Resolve clamp bounds from the nearest `[data-floating-bounds]` ancestor,
 * else the element itself. Mark cards/rows with `data-floating-bounds`.
 */
export function resolveFloatingBounds(target: HTMLElement): DOMRect {
  const host = target.closest("[data-floating-bounds]") as HTMLElement | null;
  return (host ?? target).getBoundingClientRect();
}

/** Anchor at the horizontal center / top edge of an element. */
export function anchorFromElement(
  element: HTMLElement,
  edge: "top" | "bottom" = "top",
): FloatingPopoverAnchor {
  const rect = element.getBoundingClientRect();
  return {
    x: rect.left + rect.width / 2,
    y: edge === "top" ? rect.top : rect.bottom,
  };
}

function viewportBounds(): DOMRect {
  return new DOMRect(0, 0, window.innerWidth, window.innerHeight);
}
