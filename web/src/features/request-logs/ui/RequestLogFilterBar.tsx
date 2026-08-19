import { RefreshCw, RotateCcw } from "lucide-react";

import type {
  RequestLogFilterOptions,
  RequestLogFilters,
  StableRequestLogFilterOption,
} from "../api/request-log-filter-contracts";
import { Button } from "@/shared/ui/Button";
import { Select, type SelectOption } from "@/shared/ui/Select";

const outcomeOptions: readonly SelectOption<string>[] = [
  { value: "", label: "全部结果" },
  { value: "success", label: "成功" },
  { value: "failed", label: "失败" },
  { value: "cancelled", label: "已取消" },
];

interface RequestLogFilterBarProps {
  filters: RequestLogFilters;
  options: RequestLogFilterOptions;
  onChange: (filters: RequestLogFilters) => void;
  onRefresh: () => void;
  refreshing: boolean;
}

export function RequestLogFilterBar({
  filters,
  options,
  onChange,
  onRefresh,
  refreshing,
}: RequestLogFilterBarProps) {
  function change(
    value: string,
    key: "outcome" | "publicModel" | "gatewayApiKeyId",
  ) {
    onChange({ ...filters, [key]: value || undefined });
  }

  return (
    <div
      className="grid w-full grid-cols-2 items-center gap-2 sm:ml-auto sm:flex sm:w-auto sm:flex-wrap"
      aria-label="请求日志筛选"
    >
      <Select
        value={filters.outcome ?? ""}
        options={outcomeOptions}
        onValueChange={(value) => change(value, "outcome")}
        aria-label="结果"
        className="w-full sm:w-32"
      />
      <Select
        value={filters.publicModel ?? ""}
        options={[
          { value: "", label: "全部模型" },
          ...options.publicModels.map((model) => ({ value: model, label: model })),
        ]}
        onValueChange={(value) => change(value, "publicModel")}
        aria-label="公开模型"
        className="w-full sm:w-48"
      />
      <Select
        value={filters.gatewayApiKeyId ?? ""}
        options={[
          { value: "", label: "全部网关 Key" },
          ...options.gatewayApiKeys.map(optionWithDeletedLabel),
        ]}
        onValueChange={(value) => change(value, "gatewayApiKeyId")}
        aria-label="Gateway API Key"
        className="w-full sm:w-48"
      />
      <div className="grid min-w-0 grid-cols-2 gap-2 sm:flex">
        <Button
          className="w-full sm:w-auto"
          variant="ghost"
          size="lg"
          onClick={() => onChange({})}
          aria-label="重置请求日志筛选"
        >
          <RotateCcw size={14} />
          重置
        </Button>
        <Button
          className="w-full sm:w-auto"
          variant="ghost"
          size="lg"
          onClick={onRefresh}
          disabled={refreshing}
        >
          <RefreshCw size={14} className={refreshing ? "animate-spin" : undefined} />
          刷新
        </Button>
      </div>
    </div>
  );
}

function optionWithDeletedLabel(option: StableRequestLogFilterOption): SelectOption<string> {
  return {
    value: option.id,
    label: option.deleted ? `已删除 · ${option.label}` : option.label,
  };
}
