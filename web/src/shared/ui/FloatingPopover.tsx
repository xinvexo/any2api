import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { cn } from "@/shared/lib/cn";

/** Viewport point the bubble should aim at. */
export interface FloatingPopoverAnchor {
  x: number;
  y: number;
}

export interface FloatingPopoverProps {
  open: boolean;
  /** Target in viewport coordinates (usually element center-top). */
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
}

const EDGE_PAD = 8;
const DEFAULT_GAP = 8;

/**
 * Shared floating hover/focus hint — plain rounded bubble, no caret tip.
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
    // Card/row clamps are useful horizontally, but short rows cannot fit a
    // bubble — expand any axis that is too tight so we never crush the tip
    // into the stats text underneath.
    const clamp = expandTightBounds(bounds, tip.width, tip.height);
    const minLeft = clamp.left + EDGE_PAD;
    const maxLeft = Math.max(minLeft, clamp.right - EDGE_PAD - tip.width);
    const left = Math.min(Math.max(anchor.x - tip.width / 2, minLeft), maxLeft);

    const aboveTop = anchor.y - gap - tip.height;
    const belowTop = anchor.y + gap;
    const minTop = clamp.top + EDGE_PAD;
    const maxTop = Math.max(minTop, clamp.bottom - EDGE_PAD - tip.height);

    let top: number;
    if (placement === "below") {
      top = Math.min(Math.max(belowTop, minTop), maxTop);
    } else if (placement === "above") {
      top = Math.min(Math.max(aboveTop, minTop), maxTop);
    } else if (aboveTop < minTop) {
      top = Math.min(Math.max(belowTop, minTop), maxTop);
    } else {
      top = Math.min(Math.max(aboveTop, minTop), maxTop);
    }

    setLayout({ left, top });
  }, [open, anchor, bounds, placement, gap, children]);

  if (!open || !anchor || typeof document === "undefined") {
    return null;
  }

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
          "relative rounded-[11px] border border-white/50 bg-white/80 shadow-panel",
          "px-2.5 py-1.5 text-[11px] leading-4 text-primary",
          "backdrop-blur-xl backdrop-saturate-150",
          "dark:border-white/10 dark:bg-white/12",
          className,
        )}
      >
        {children}
      </div>
    </div>,
    document.body,
  );
}

/**
 * Resolve clamp bounds from the nearest `[data-floating-bounds]` ancestor,
 * else the element itself. Mark cards/rows with `data-floating-bounds`.
 */
// Pure geometry helper intentionally shares the component module.
// eslint-disable-next-line react-refresh/only-export-components
export function resolveFloatingBounds(target: HTMLElement): DOMRect {
  const host = target.closest("[data-floating-bounds]") as HTMLElement | null;
  return (host ?? target).getBoundingClientRect();
}

/** Anchor at the horizontal center / top edge of an element. */
// Pure geometry helper intentionally shares the component module.
// eslint-disable-next-line react-refresh/only-export-components
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

/**
 * Prefer the caller clamp (card/row), but fall back to the viewport on any
 * axis that cannot fit the measured tip plus edge padding.
 */
function expandTightBounds(
  bounds: DOMRect | null,
  tipWidth: number,
  tipHeight: number,
): DOMRect {
  const viewport = viewportBounds();
  if (!bounds) {
    return viewport;
  }

  const needWidth = tipWidth + EDGE_PAD * 2;
  const needHeight = tipHeight + EDGE_PAD * 2;
  const left = bounds.width < needWidth ? viewport.left : bounds.left;
  const right = bounds.width < needWidth ? viewport.right : bounds.right;
  const top = bounds.height < needHeight ? viewport.top : bounds.top;
  const bottom = bounds.height < needHeight ? viewport.bottom : bounds.bottom;
  return new DOMRect(left, top, right - left, bottom - top);
}
