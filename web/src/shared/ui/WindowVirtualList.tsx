import { useWindowVirtualizer } from "@tanstack/react-virtual";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type Key,
  type ReactNode,
} from "react";

import { cn } from "@/shared/lib/cn";

const useBrowserLayoutEffect = typeof window === "undefined" ? useEffect : useLayoutEffect;

interface WindowVirtualListProps<T> {
  items: readonly T[];
  getItemKey: (item: T) => Key;
  renderItem: (item: T) => ReactNode;
  ariaLabel: string;
  estimateItemHeight: number;
  gap?: number;
  overscan?: number;
  className?: string;
  getItemClassName?: (item: T) => string | undefined;
}

export function WindowVirtualList<T>({
  items,
  getItemKey,
  renderItem,
  ariaLabel,
  estimateItemHeight,
  gap = 8,
  overscan = 8,
  className,
  getItemClassName,
}: WindowVirtualListProps<T>) {
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollMargin, setScrollMargin] = useState(0);
  const updateScrollMargin = useCallback(() => {
    const list = listRef.current;
    if (!list) return;
    const next = list.getBoundingClientRect().top + window.scrollY;
    setScrollMargin((current) => (current === next ? current : next));
  }, []);

  const virtualizer = useWindowVirtualizer<HTMLDivElement>({
    count: items.length,
    estimateSize: () => estimateItemHeight,
    getItemKey: (index) => getItemKey(items[index]),
    gap,
    overscan,
    scrollMargin,
    useFlushSync: false,
    initialRect: { width: 390, height: 844 },
    measureElement: (element, entry) => {
      const measured = entry?.borderBoxSize?.[0]?.blockSize
        ?? element.getBoundingClientRect().height;
      return measured > 0 ? measured : estimateItemHeight;
    },
  });

  useBrowserLayoutEffect(updateScrollMargin);

  useEffect(() => {
    window.addEventListener("resize", updateScrollMargin);
    return () => window.removeEventListener("resize", updateScrollMargin);
  }, [updateScrollMargin]);

  return (
    <div
      ref={listRef}
      role="list"
      aria-label={ariaLabel}
      className={cn("relative w-full", className)}
      style={{ height: `${virtualizer.getTotalSize()}px` }}
    >
      {virtualizer.getVirtualItems().map((virtualItem) => {
        const item = items[virtualItem.index];
        return (
          <div
            key={virtualItem.key}
            ref={virtualizer.measureElement}
            data-index={virtualItem.index}
            role="listitem"
            aria-posinset={virtualItem.index + 1}
            aria-setsize={items.length}
            className={cn(
              "absolute left-0 top-0 w-full",
              getItemClassName?.(item),
            )}
            style={{
              transform: `translateY(${virtualItem.start - scrollMargin}px)`,
            }}
          >
            {renderItem(item)}
          </div>
        );
      })}
    </div>
  );
}
