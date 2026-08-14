import { ArrowRight, RotateCcw } from "lucide-react";
import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";

import type {
  RequestLogFilterOptions,
  RequestLogFilters,
  StableRequestLogFilterOption,
} from "../api/request-log-filter-contracts";
import { Button } from "@/shared/ui/Button";
import { Select, type SelectOption } from "@/shared/ui/Select";
import { controlClass } from "@/shared/ui/form-control";

const outcomeOptions: readonly SelectOption<string>[] = [
  { value: "", label: "全部结果" },
  { value: "success", label: "成功" },
  { value: "failed", label: "失败" },
  { value: "cancelled", label: "已取消" },
];

const operationOptions: readonly SelectOption<string>[] = [
  { value: "", label: "全部操作" },
  { value: "responses", label: "Responses" },
  { value: "responses_compact", label: "Responses Compact" },
  { value: "chat_completions", label: "Chat Completions" },
  { value: "images_generations", label: "Images Generations" },
  { value: "images_edits", label: "Images Edits" },
  { value: "messages", label: "Messages" },
  { value: "messages_count_tokens", label: "Messages Count Tokens" },
];

interface RequestLogFilterBarProps {
  filters: RequestLogFilters;
  options: RequestLogFilterOptions;
  onChange: (filters: RequestLogFilters) => void;
}

export function RequestLogFilterBar({
  filters,
  options,
  onChange,
}: RequestLogFilterBarProps) {
  const navigate = useNavigate();
  const [requestId, setRequestId] = useState("");
  const [requestIdError, setRequestIdError] = useState(false);
  const upstreamValue = filters.credentialId
    ? `credential:${filters.credentialId}`
    : filters.oauthAccountId
      ? `oauth:${filters.oauthAccountId}`
      : "";
  const upstreamOptions = [
    { value: "", label: "全部上游凭据" },
    ...options.providerCredentials.map((option) =>
      optionWithDeletedLabel(option, "Provider", "credential"),
    ),
    ...options.oauthAccounts.map((option) =>
      optionWithDeletedLabel(option, "OAuth", "oauth"),
    ),
  ];

  function submitRequestId(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const value = requestId.trim();
    if (!isUuid(value)) {
      setRequestIdError(true);
      return;
    }
    setRequestIdError(false);
    navigate(`/logs/${encodeURIComponent(value)}`);
  }

  function change(
    value: string,
    key: "outcome" | "operation" | "publicModel" | "gatewayApiKeyId",
  ) {
    onChange({ ...filters, [key]: value || undefined });
  }

  function changeUpstream(value: string) {
    const [kind, id] = value.split(":", 2);
    onChange({
      ...filters,
      credentialId: kind === "credential" ? id : undefined,
      oauthAccountId: kind === "oauth" ? id : undefined,
    });
  }

  return (
    <div className="shrink-0 space-y-2 border-b border-subtle py-3">
      <form className="flex max-w-xl items-center gap-2" onSubmit={submitRequestId}>
        <label htmlFor="request-log-request-id" className="sr-only">
          Request ID
        </label>
        <input
          id="request-log-request-id"
          value={requestId}
          onChange={(event) => {
            setRequestId(event.target.value);
            setRequestIdError(false);
          }}
          placeholder="Request ID"
          className={controlClass(requestIdError, "max-w-sm")}
          aria-invalid={requestIdError}
        />
        <Button type="submit" size="sm" variant="secondary">
          <ArrowRight size={14} />
          定位
        </Button>
        {requestIdError ? (
          <span className="text-[12px] text-danger" role="alert">
            Request ID 无效
          </span>
        ) : null}
      </form>

      <div className="flex flex-wrap items-center gap-2" aria-label="请求日志筛选">
        <Select
          value={filters.outcome ?? ""}
          options={outcomeOptions}
          onValueChange={(value) => change(value, "outcome")}
          aria-label="结果"
          className="w-32"
        />
        <Select
          value={filters.operation ?? ""}
          options={operationOptions}
          onValueChange={(value) => change(value, "operation")}
          aria-label="操作"
          className="w-44"
        />
        <Select
          value={filters.publicModel ?? ""}
          options={[
            { value: "", label: "全部模型" },
            ...options.publicModels.map((model) => ({ value: model, label: model })),
          ]}
          onValueChange={(value) => change(value, "publicModel")}
          aria-label="公开模型"
          className="w-44"
        />
        <Select
          value={filters.gatewayApiKeyId ?? ""}
          options={[
            { value: "", label: "全部网关 Key" },
            ...options.gatewayApiKeys.map((option) => optionWithDeletedLabel(option, "Key")),
          ]}
          onValueChange={(value) => change(value, "gatewayApiKeyId")}
          aria-label="Gateway API Key"
          className="w-48"
        />
        <Select
          value={upstreamValue}
          options={upstreamOptions}
          onValueChange={changeUpstream}
          aria-label="上游凭据"
          className="w-56"
        />
        <Button
          variant="ghost"
          size="sm"
          onClick={() => onChange({})}
          aria-label="清除请求日志筛选"
        >
          <RotateCcw size={14} />
          清除
        </Button>
      </div>
    </div>
  );
}

function optionWithDeletedLabel(
  option: StableRequestLogFilterOption,
  kind: string,
  valuePrefix?: "credential" | "oauth",
): SelectOption<string> {
  return {
    value: valuePrefix ? `${valuePrefix}:${option.id}` : option.id,
    label: option.deleted ? `已删除 · ${option.label}` : `${kind} · ${option.label}`,
  };
}

function isUuid(value: string) {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);
}
