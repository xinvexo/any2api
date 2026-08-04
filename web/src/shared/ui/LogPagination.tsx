import { ChevronLeft, ChevronRight } from "lucide-react";

import {
  logPageCount,
  LOG_PAGE_SIZE_OPTIONS,
  type LogPageSize,
} from "@/shared/lib/log-pagination";
import { IconButton } from "@/shared/ui/IconButton";
import { Select } from "@/shared/ui/Select";

export function LogPagination({
  page,
  pageSize,
  total,
  hasNextPage,
  onPageChange,
  onPageSizeChange,
}: {
  page: number;
  pageSize: LogPageSize;
  total: number;
  hasNextPage: boolean;
  onPageChange: (page: number) => void;
  onPageSizeChange: (size: LogPageSize) => void;
}) {
  const totalPages = logPageCount(total, pageSize);
  const safePage = Math.min(Math.max(1, page), totalPages);

  return (
    <div className="flex h-8 min-w-0 flex-wrap items-center gap-1.5 text-[12px] text-secondary">
      <div className="flex items-center gap-1.5">
        <Select
          className="w-auto min-w-28"
          value={pageSize}
          options={LOG_PAGE_SIZE_OPTIONS.map((size) => ({
            value: size,
            label: `${size} 条/页`,
          }))}
          aria-label="每页条数"
          onValueChange={onPageSizeChange}
        />
      </div>
      <span className="tabular-nums text-tertiary">共 {total} 条</span>
      <div className="flex items-center gap-0.5">
        <IconButton
          label="上一页"
          disabled={page <= 1}
          onClick={() => onPageChange(page > totalPages ? totalPages : safePage - 1)}
        >
          <ChevronLeft size={16} strokeWidth={1.75} />
        </IconButton>
        <span className="min-w-[3.25rem] text-center tabular-nums text-primary">
          {safePage}/{totalPages}
        </span>
        <IconButton
          label="下一页"
          disabled={!hasNextPage}
          onClick={() => onPageChange(safePage + 1)}
        >
          <ChevronRight size={16} strokeWidth={1.75} />
        </IconButton>
      </div>
    </div>
  );
}
