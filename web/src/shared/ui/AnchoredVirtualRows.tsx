import {
  observeElementRect as observeVirtualElementRect,
  useVirtualizer,
} from "@tanstack/react-virtual";
import { useLayoutEffect, useRef, type ReactNode } from "react";

interface AnchoredVirtualRowsProps {
  itemIds: readonly string[];
  rowHeight: number;
  followingLatest: boolean;
  hasMore: boolean;
  loadingMore: boolean;
  historyLoaderKey: string;
  initialWidth: number;
  ariaLabel: string;
  onFollowingLatestChange: (following: boolean) => void;
  onLoadMore: () => void;
  renderRow: (index: number) => ReactNode;
  renderHistoryLoader: (loading: boolean) => ReactNode;
}

export function AnchoredVirtualRows({
  itemIds,
  rowHeight,
  followingLatest,
  hasMore,
  loadingMore,
  historyLoaderKey,
  initialWidth,
  ariaLabel,
  onFollowingLatestChange,
  onLoadMore,
  renderRow,
  renderHistoryLoader,
}: AnchoredVirtualRowsProps) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const anchorRef = useRef({ ids: [] as string[], scrollTop: 0 });
  // TanStack Virtual exposes a mutable controller that React Compiler must not memoize.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer<HTMLDivElement, HTMLDivElement>({
    count: itemIds.length + (hasMore ? 1 : 0),
    getScrollElement: () => viewportRef.current,
    estimateSize: () => rowHeight,
    getItemKey: (index) => itemIds[index] ?? historyLoaderKey,
    overscan: 10,
    useFlushSync: false,
    initialRect: { width: initialWidth, height: 640 },
    observeElementRect: (instance, callback) =>
      observeVirtualElementRect(instance, (rect) =>
        callback({ width: rect.width, height: rect.height > 0 ? rect.height : 640 }),
      ),
  });

  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) {
      return;
    }
    const previous = anchorRef.current;
    if (followingLatest) {
      viewport.scrollTop = 0;
    } else if (previous.ids.length > 0 && itemIds.length > 0) {
      const oldFirstInNext = itemIds.indexOf(previous.ids[0]);
      const nextFirstInOld = previous.ids.indexOf(itemIds[0]);
      if (previous.scrollTop <= rowHeight) {
        viewport.scrollTop = 0;
      } else if (oldFirstInNext > 0) {
        viewport.scrollTop = previous.scrollTop + oldFirstInNext * rowHeight;
      } else if (nextFirstInOld > 0) {
        viewport.scrollTop = Math.max(
          0,
          previous.scrollTop - nextFirstInOld * rowHeight,
        );
      }
    }
    anchorRef.current = { ids: [...itemIds], scrollTop: viewport.scrollTop };
  }, [followingLatest, itemIds, rowHeight]);

  const handleScroll = () => {
    const viewport = viewportRef.current;
    if (!viewport) {
      return;
    }
    anchorRef.current.scrollTop = viewport.scrollTop;
    onFollowingLatestChange(viewport.scrollTop <= rowHeight);
    const remaining = virtualizer.getTotalSize() - viewport.scrollTop - viewport.clientHeight;
    if (hasMore && !loadingMore && remaining <= rowHeight * 10) {
      onLoadMore();
    }
  };

  return (
    <div
      ref={viewportRef}
      role="rowgroup"
      aria-label={ariaLabel}
      // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
      tabIndex={0}
      className="focus-ring min-h-0 flex-1 overflow-y-scroll outline-none [scrollbar-gutter:stable]"
      onScroll={handleScroll}
    >
      <div className="relative w-full" style={{ height: `${virtualizer.getTotalSize()}px` }}>
        {virtualizer.getVirtualItems().map((virtualRow) => (
          <div
            key={virtualRow.key}
            className="absolute left-0 top-0 w-full"
            style={{ height: rowHeight, transform: `translateY(${virtualRow.start}px)` }}
          >
            {virtualRow.index < itemIds.length
              ? renderRow(virtualRow.index)
              : renderHistoryLoader(loadingMore)}
          </div>
        ))}
      </div>
    </div>
  );
}
