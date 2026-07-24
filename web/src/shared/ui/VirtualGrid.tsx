import {
  observeElementRect as observeVirtualElementRect,
  useVirtualizer,
} from "@tanstack/react-virtual";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type Key,
  type ReactNode,
} from "react";

import { cn } from "@/shared/lib/cn";

export interface VirtualGridProps<T> {
  items: readonly T[];
  getItemKey: (item: T) => Key;
  renderItem: (item: T) => ReactNode;
  ariaLabel: string;
  collectionKey: Key;
  estimateRowHeight: number;
  minItemWidth?: number;
  maxColumns?: number;
  gap?: number;
  overscanRows?: number;
  className?: string;
}

export function VirtualGrid<T>({
  items,
  getItemKey,
  renderItem,
  ariaLabel,
  collectionKey,
  estimateRowHeight,
  minItemWidth = 260,
  maxColumns = 3,
  gap = 12,
  overscanRows = 2,
  className,
}: VirtualGridProps<T>) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const width = useElementWidth(viewportRef);
  const columnCount = width <= 0
    ? 1
    : Math.max(
        1,
        Math.min(maxColumns, Math.floor((width + gap) / (minItemWidth + gap))),
      );
  const rowCount = Math.ceil(items.length / columnCount);
  const rowKeys = useMemo(
    () =>
      Array.from({ length: rowCount }, (_, rowIndex) => {
        const item = items[rowIndex * columnCount];
        return `${String(collectionKey)}:${columnCount}:${String(getItemKey(item))}`;
      }),
    [collectionKey, columnCount, getItemKey, items, rowCount],
  );
  // TanStack Virtual intentionally exposes a mutable controller; React Compiler
  // must not memoize this hook result across option changes.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer<HTMLDivElement, HTMLDivElement>({
    count: rowCount,
    getScrollElement: () => viewportRef.current,
    estimateSize: () => estimateRowHeight,
    getItemKey: (index) => rowKeys[index],
    gap,
    overscan: overscanRows,
    useFlushSync: false,
    initialRect: { width: 0, height: 640 },
    observeElementRect: (instance, callback) =>
      observeVirtualElementRect(instance, (rect) =>
        callback({
          width: rect.width,
          height: rect.height > 0 ? rect.height : 640,
        }),
      ),
    measureElement: (element, entry) => {
      const measured = entry?.borderBoxSize?.[0]?.blockSize
        ?? element.getBoundingClientRect().height;
      return measured > 0 ? measured : estimateRowHeight;
    },
  });

  useEffect(() => {
    virtualizer.measure();
  }, [columnCount, virtualizer]);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    viewport.scrollTop = 0;
    viewport.dispatchEvent(new Event("scroll"));
  }, [collectionKey]);

  return (
    <div
      ref={viewportRef}
      role="region"
      aria-label={`${ariaLabel}滚动区域`}
      // A scroll region must be keyboard-focusable even though region itself is
      // not an interactive ARIA role.
      // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
      tabIndex={0}
      className={cn(
        "max-h-[min(72vh,52rem)] overflow-y-auto overflow-x-hidden pr-1 outline-none [scrollbar-gutter:stable] focus-visible:ring-2 focus-visible:ring-accent/45",
        className,
      )}
    >
      <div
        role="list"
        aria-label={ariaLabel}
        className="relative w-full"
        style={{ height: `${virtualizer.getTotalSize()}px` }}
      >
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const start = virtualRow.index * columnCount;
          const rowItems = items.slice(start, start + columnCount);
          return (
            <div
              key={virtualRow.key}
              ref={virtualizer.measureElement}
              data-index={virtualRow.index}
              className="absolute left-0 top-0 grid w-full items-stretch"
              style={{
                gap: `${gap}px`,
                gridTemplateColumns: `repeat(${columnCount}, minmax(0, 1fr))`,
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
              {rowItems.map((item, itemOffset) => {
                const itemIndex = start + itemOffset;
                return (
                  <div
                    key={getItemKey(item)}
                    role="listitem"
                    aria-posinset={itemIndex + 1}
                    aria-setsize={items.length}
                    className="h-full min-w-0"
                  >
                    {renderItem(item)}
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function useElementWidth(ref: React.RefObject<HTMLElement | null>) {
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const update = () => setWidth(element.clientWidth);
    update();
    if (typeof ResizeObserver !== "function") {
      window.addEventListener("resize", update);
      return () => window.removeEventListener("resize", update);
    }
    const observer = new ResizeObserver((entries) => {
      const next = entries[0]?.contentRect.width ?? element.clientWidth;
      setWidth(next);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);

  return width;
}
