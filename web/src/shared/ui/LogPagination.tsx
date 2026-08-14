import {
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  MoveRight,
} from "lucide-react";
import { useRef, type FormEvent } from "react";

import {
  logPageCount,
  LOG_PAGE_SIZE_OPTIONS,
  type LogPageSize,
} from "@/shared/lib/log-pagination";
import { controlClass } from "@/shared/ui/form-control";
import { IconButton } from "@/shared/ui/IconButton";
import { Select } from "@/shared/ui/Select";

export function LogPagination({
  page,
  pageSize,
  total,
  hasNextPage,
  disabled = false,
  onPageChange,
  onPageSizeChange,
}: {
  page: number;
  pageSize: LogPageSize;
  total: number;
  hasNextPage: boolean;
  disabled?: boolean;
  onPageChange: (page: number) => void;
  onPageSizeChange: (size: LogPageSize) => void;
}) {
  const totalPages = logPageCount(total, pageSize);
  const safePage = Math.min(Math.max(1, page), totalPages);
  const pageInputRef = useRef<HTMLInputElement>(null);

  const submitPage = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const input = pageInputRef.current;
    const rawPage = input?.value.trim() ?? "";
    const parsed = rawPage === "" ? Number.NaN : Number(rawPage);
    const target = Number.isInteger(parsed)
      ? Math.min(Math.max(1, parsed), totalPages)
      : safePage;
    if (input) {
      input.value = String(target);
    }
    if (target !== safePage) {
      onPageChange(target);
    }
  };

  return (
    <div className="flex min-h-8 min-w-0 flex-wrap items-center justify-end gap-x-2 gap-y-1.5 text-[12px] text-secondary">
      <Select
        className="w-auto min-w-28"
        value={pageSize}
        options={LOG_PAGE_SIZE_OPTIONS.map((size) => ({
          value: size,
          label: `${size} 条/页`,
        }))}
        aria-label="每页条数"
        disabled={disabled}
        onValueChange={onPageSizeChange}
      />
      <span className="whitespace-nowrap tabular-nums text-tertiary">共 {total} 条</span>
      <div className="flex min-w-0 items-center gap-0.5">
        <IconButton
          label="首页"
          title="首页"
          size="sm"
          disabled={disabled || safePage <= 1}
          onClick={() => onPageChange(1)}
        >
          <ChevronsLeft size={15} strokeWidth={1.75} />
        </IconButton>
        <IconButton
          label="上一页"
          title="上一页"
          size="sm"
          disabled={disabled || safePage <= 1}
          onClick={() => onPageChange(safePage - 1)}
        >
          <ChevronLeft size={15} strokeWidth={1.75} />
        </IconButton>

        <form className="flex items-center gap-1" onSubmit={submitPage}>
          <input
            key={`${safePage}:${totalPages}`}
            ref={pageInputRef}
            className={controlClass(
              false,
              "w-14 px-1.5 text-center tabular-nums [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none",
            )}
            type="number"
            inputMode="numeric"
            min={1}
            max={totalPages}
            step={1}
            defaultValue={safePage}
            aria-label={`页码，共 ${totalPages} 页`}
            disabled={disabled}
            onBlur={(event) => {
              if (event.currentTarget.value.trim() === "") {
                event.currentTarget.value = String(safePage);
              }
            }}
          />
          <span className="min-w-10 whitespace-nowrap tabular-nums text-primary">
            / {totalPages}
          </span>
          <IconButton
            label="跳转到页码"
            title="跳转到页码"
            size="sm"
            type="submit"
            disabled={disabled || totalPages <= 1}
          >
            <MoveRight size={15} strokeWidth={1.75} />
          </IconButton>
        </form>

        <IconButton
          label="下一页"
          title="下一页"
          size="sm"
          disabled={disabled || !hasNextPage}
          onClick={() => onPageChange(safePage + 1)}
        >
          <ChevronRight size={15} strokeWidth={1.75} />
        </IconButton>
        <IconButton
          label="末页"
          title="末页"
          size="sm"
          disabled={disabled || safePage >= totalPages}
          onClick={() => onPageChange(totalPages)}
        >
          <ChevronsRight size={15} strokeWidth={1.75} />
        </IconButton>
      </div>
    </div>
  );
}
