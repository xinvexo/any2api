import { useLayoutEffect, useRef } from "react";

import { cn } from "@/shared/lib/cn";

interface SlidingSelectionIndicatorProps {
  selected: string;
  className?: string;
}

export function SlidingSelectionIndicator({
  selected,
  className,
}: SlidingSelectionIndicatorProps) {
  const indicatorRef = useRef<HTMLSpanElement>(null);

  useLayoutEffect(() => {
    const indicator = indicatorRef.current;
    const container = indicator?.parentElement;
    if (!indicator || !container) {
      return;
    }
    const activeItem = findActiveItem(container, selected);
    if (!activeItem) {
      indicator.dataset.ready = "false";
      return;
    }

    const measure = () => {
      const containerRect = container.getBoundingClientRect();
      const itemRect = activeItem.getBoundingClientRect();
      const left = itemRect.left - containerRect.left + container.scrollLeft - container.clientLeft;
      const top = itemRect.top - containerRect.top + container.scrollTop - container.clientTop;
      indicator.style.width = `${itemRect.width}px`;
      indicator.style.height = `${itemRect.height}px`;
      indicator.style.transform = `translate3d(${left}px, ${top}px, 0)`;
      indicator.dataset.ready = "true";
    };

    measure();
    container.addEventListener("scroll", measure, { passive: true });
    window.addEventListener("resize", measure);
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(measure) : null;
    observer?.observe(container);
    observer?.observe(activeItem);

    return () => {
      observer?.disconnect();
      container.removeEventListener("scroll", measure);
      window.removeEventListener("resize", measure);
    };
  }, [selected]);

  return (
    <span
      ref={indicatorRef}
      aria-hidden="true"
      data-sliding-selection-indicator="true"
      data-active-value={selected}
      data-ready="false"
      className={cn(
        "pointer-events-none absolute left-0 top-0 z-0 opacity-0",
        "transition-[transform,width,height,opacity] duration-300",
        "[transition-timing-function:cubic-bezier(0.22,1,0.36,1)]",
        "data-[ready=true]:opacity-100",
        className,
      )}
    />
  );
}

function findActiveItem(container: HTMLElement, selected: string) {
  return Array.from(container.querySelectorAll<HTMLElement>("[data-sliding-selection-item]"))
    .find((item) => item.dataset.slidingSelectionItem === selected);
}
