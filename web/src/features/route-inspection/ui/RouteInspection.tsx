import { RefreshCw, Search, Waypoints } from "lucide-react";
import { useMemo, useState } from "react";

import type {
  RouteInspectionCandidateGroup,
  RouteInspectionItem,
  RouteInspectionStatus,
  RouteProtocolDialect,
  RouteProtocolOperation,
  RouteProviderKind,
} from "../api/route-inspection-contracts";
import { getRouteInspectionErrorMessage } from "../model/route-inspection-error";
import { useRouteInspection } from "../model/use-route-inspection";
import { cn } from "@/shared/lib/cn";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";
import { Select } from "@/shared/ui/Select";
import { Surface } from "@/shared/ui/Surface";
import { controlClass } from "@/shared/ui/form-control";

type StatusFilter = "all" | RouteInspectionStatus;

const STATUS_OPTIONS = [
  { value: "all", label: "全部状态" },
  { value: "available", label: "可用" },
  { value: "no_enabled_candidate", label: "无启用候选" },
] satisfies ReadonlyArray<{ value: StatusFilter; label: string }>;
const EMPTY_ITEMS: RouteInspectionItem[] = [];

export function RouteInspection() {
  const inspection = useRouteInspection();
  const [modelQuery, setModelQuery] = useState("");
  const [status, setStatus] = useState<StatusFilter>("all");
  const items = inspection.data?.items ?? EMPTY_ITEMS;
  const filtered = useMemo(() => {
    const model = modelQuery.trim();
    return items.filter(
      (item) =>
        (model.length === 0 || item.publicModel === model) &&
        (status === "all" || item.status === status),
    );
  }, [items, modelQuery, status]);

  async function refresh() {
    const result = await inspection.refetch();
    if (result.isSuccess) notify.success("路由检查已刷新");
  }

  const data = inspection.data;

  return (
    <div
      className="flex flex-1 flex-col md:h-full md:min-h-0 md:overflow-hidden"
      aria-busy={inspection.isFetching}
    >
      <header className="flex min-h-8 shrink-0 flex-wrap items-center gap-2 border-b border-subtle pb-3">
        <label className="relative min-w-[12rem] flex-1 sm:max-w-sm">
          <span className="sr-only">精确模型搜索</span>
          <Search
            size={14}
            className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-tertiary"
            aria-hidden="true"
          />
          <input
            className={controlClass(false, "pl-8 pr-3")}
            value={modelQuery}
            placeholder="精确模型名"
            disabled={!data}
            onChange={(event) => setModelQuery(event.target.value)}
          />
        </label>
        <div className="ml-auto flex shrink-0 items-center gap-2">
          <Select
            value={status}
            options={STATUS_OPTIONS}
            onValueChange={setStatus}
            aria-label="状态筛选"
            className="w-36"
            disabled={!data}
          />
          <Button variant="ghost" onClick={() => void refresh()} disabled={inspection.isFetching}>
            <RefreshCw size={14} className={inspection.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
        </div>
      </header>

      {inspection.isError && data ? (
        <Surface className="mt-3 shrink-0 border-warning/40 p-4 text-sm text-secondary" role="status">
          刷新失败，当前仍显示最近一次有效数据：
          {getRouteInspectionErrorMessage(inspection.error)}
        </Surface>
      ) : null}

      <div className="management-scroll-viewport min-h-0 flex-1 overflow-y-auto pt-3 [scrollbar-gutter:stable]">
        {inspection.isPending && !data ? (
          <Surface
            className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary"
            aria-busy="true"
          >
            正在读取路由配置
          </Surface>
        ) : !data ? (
          <Surface className="p-6" role="alert">
            <p className="font-semibold">无法读取路由检查</p>
            <p className="mt-2 text-sm text-secondary">
              {getRouteInspectionErrorMessage(inspection.error)}
            </p>
            <Button className="mt-5" onClick={() => void refresh()} disabled={inspection.isFetching}>
              <RefreshCw size={14} />
              重试
            </Button>
          </Surface>
        ) : filtered.length === 0 ? (
          <div className="flex min-h-52 flex-col items-center justify-center px-6 py-10 text-center" role="status">
            <Waypoints size={22} className="text-tertiary" aria-hidden="true" />
            <p className="mt-3 text-[13px] font-medium">
              {items.length === 0 ? "当前没有允许的公开模型" : "没有匹配的路由"}
            </p>
          </div>
        ) : (
          <div role="list" aria-label="路由检查结果" className="grid gap-3 xl:grid-cols-2">
            {filtered.map((item) => (
              <RouteItem key={`${item.publicModel}:${item.ingressProtocol}`} item={item} />
            ))}
          </div>
        )}
      </div>

      {data ? (
        <div className="flex shrink-0 items-center justify-between gap-3 border-t border-subtle pt-3 text-[12px] text-secondary">
          <p>
            显示 <span className="tabular-nums text-primary">{filtered.length}</span> / {items.length}
          </p>
          <p className="tabular-nums">配置版本 {data.configRevision}</p>
        </div>
      ) : null}
    </div>
  );
}

function RouteItem({ item }: { item: RouteInspectionItem }) {
  return (
    <article
      role="listitem"
      aria-label={`${item.publicModel} 路由`}
      className="min-w-0 rounded-[14px] bg-surface-muted/45 p-4"
    >
      <header className="flex min-w-0 flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="break-all font-mono text-[13px] font-semibold text-primary">
            {item.publicModel}
          </p>
          <p className="mt-1 text-[11px] text-tertiary">
            {dialectLabel(item.ingressProtocol)}
            <span className="mx-1.5">·</span>
            <span>{item.published ? "已发布" : "未发布"}</span>
          </p>
        </div>
        <div className="flex shrink-0 items-center justify-end text-[11px]">
          <Status status={item.status} />
        </div>
      </header>

      <div className="mt-3 border-t border-subtle/70">
        {item.operations.map((operation) => (
          <div
            key={operation.operation}
            className="grid min-w-0 gap-2 border-b border-subtle/50 py-2.5 last:border-b-0 sm:grid-cols-[7rem_minmax(0,1fr)]"
          >
            <p className="text-[12px] font-medium text-secondary">
              {operationLabel(operation.operation)}
            </p>
            {operation.candidateGroups.length === 0 ? (
              <p className="text-[12px] text-tertiary">无启用候选</p>
            ) : (
              <ul className="min-w-0 space-y-1.5">
                {operation.candidateGroups.map((group) => (
                  <CandidateGroup
                    key={`${group.providerKind}:${group.providerEndpointId ?? "oauth"}:${group.upstreamProtocolDialect}`}
                    group={group}
                  />
                ))}
              </ul>
            )}
          </div>
        ))}
      </div>
    </article>
  );
}

function CandidateGroup({ group }: { group: RouteInspectionCandidateGroup }) {
  return (
    <li className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-start gap-3 text-[12px]">
      <p className="min-w-0 text-secondary">
        <span className="font-medium text-primary">{providerLabel(group.providerKind)}</span>
        <span className="mx-1.5 text-tertiary">·</span>
        <span className="break-words">{group.providerEndpointName ?? "OAuth"}</span>
        <span className="mx-1.5 text-tertiary">·</span>
        <span>{dialectLabel(group.upstreamProtocolDialect)}</span>
      </p>
      <p className="whitespace-nowrap font-medium tabular-nums text-primary">
        {group.enabledCandidateCount} 个
      </p>
    </li>
  );
}

function Status({ status }: { status: RouteInspectionStatus }) {
  const presentation = {
    available: { label: "可用", tone: "text-success" },
    no_enabled_candidate: { label: "无启用候选", tone: "text-danger" },
  }[status];
  return (
    <span className={cn("inline-flex items-center gap-1.5 font-medium", presentation.tone)}>
      <span className="size-1.5 rounded-full bg-current" aria-hidden="true" />
      {presentation.label}
    </span>
  );
}

function providerLabel(provider: RouteProviderKind) {
  return { openai: "OpenAI", codex: "Codex", claude: "Claude", grok: "Grok", kimi: "Kimi" }[
    provider
  ];
}

function dialectLabel(dialect: RouteProtocolDialect) {
  return {
    openai_responses: "OpenAI Responses",
    openai_chat_completions: "Chat Completions",
    openai_images: "OpenAI Images",
    anthropic_messages: "Anthropic Messages",
  }[dialect];
}

function operationLabel(operation: RouteProtocolOperation) {
  return {
    responses: "响应生成",
    responses_compact: "响应压缩",
    alpha_search: "联网搜索",
    chat_completions: "聊天补全",
    images_generations: "图像生成",
    images_edits: "图像编辑",
    messages: "消息",
    messages_count_tokens: "Token 计数",
  }[operation];
}
