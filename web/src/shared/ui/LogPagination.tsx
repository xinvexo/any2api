import { ChevronLeft, ChevronRight } from "lucide-react";

import {
  isLogPageSize,
  logPageCount,
  LOG_PAGE_SIZE_OPTIONS,
  type LogPageSize,
} from "@/shared/lib/log-pagination";
import { IconButton } from "@/shared/ui/IconButton";
import { selectClass } from "@/shared/ui/form-control";

export function LogPagination({
  page,
  pageSize,
  total,
  onPageChange,
  onPageSizeChange,
}: {
  page: number;
  pageSize: LogPageSize;
  total: number;
  onPageChange: (page: number) => void;
  onPageSizeChange: (size: LogPageSize) => void;
}) {
  const totalPages = logPageCount(total, pageSize);
  const safePage = Math.min(Math.max(1, page), totalPages);

  return (
    <div className="flex h-8 min-w-0 flex-wrap items-center gap-1.5 text-[12px] text-secondary">
      <label className="flex items-center gap-1.5">
        <span className="sr-only">每页条数</span>
        <select
          className={selectClass(false, "w-auto min-w-[4.5rem]")}
          value={pageSize}
          aria-label="每页条数"
          onChange={(event) => {
            const next = Number(event.target.value);
            if (isLogPageSize(next)) {
              onPageSizeChange(next);
            }
          }}
        >
          {LOG_PAGE_SIZE_OPTIONS.map((size) => (
            <option key={size} value={size}>
              {size} 条/页
            </option>
          ))}
        </select>
      </label>
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
          disabled={safePage >= totalPages}
          onClick={() => onPageChange(safePage + 1)}
        >
          <ChevronRight size={16} strokeWidth={1.75} />
        </IconButton>
      </div>
    </div>
  );
}
