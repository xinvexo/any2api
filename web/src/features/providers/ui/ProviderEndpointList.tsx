import { Plus, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";

import type {
  ProviderEndpoint,
  ProviderEndpointConfiguration,
  ProviderKind,
} from "../api/provider-contracts";
import {
  isProviderKind,
  providerKindLabel,
  PROVIDER_KIND_OPTIONS,
} from "../model/provider-kind-catalog";
import { getProviderErrorMessage } from "../model/provider-error";
import { ProviderCredentialManagement } from "./ProviderCredentialManagement";
import {
  ENDPOINT_CONTENT_GRID_CLASS,
  ProviderEndpointTableRow,
} from "./ProviderEndpointTableRow";
import { ProviderKindNav } from "./ProviderKindNav";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";
import { cn } from "@/shared/lib/cn";

interface ProviderEndpointListProps {
  configuration: ProviderEndpointConfiguration;
  pending: boolean;
  refreshing: boolean;
  actionError: unknown;
  onCreate: (kind: ProviderKind) => void;
  onRefresh: () => void;
  onEdit: (id: string) => void;
  onDelete: (endpoint: ProviderEndpoint) => void;
}

export function ProviderEndpointList({
  configuration,
  pending,
  refreshing,
  actionError,
  onCreate,
  onRefresh,
  onEdit,
  onDelete,
}: ProviderEndpointListProps) {
  const [searchParams, setSearchParams] = useSearchParams();
  const activeKeysEndpoint = searchParams.get("keys");
  const selectedKind = resolveSelectedKind(searchParams.get("kind"));
  const [query, setQuery] = useState("");
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());

  const counts = useMemo(() => {
    const next = Object.fromEntries(
      PROVIDER_KIND_OPTIONS.map((option) => [option.kind, 0]),
    ) as Record<ProviderKind, number>;
    for (const endpoint of configuration.items) {
      next[endpoint.providerKind] = (next[endpoint.providerKind] ?? 0) + 1;
    }
    return next;
  }, [configuration.items]);

  const kindItems = useMemo(
    () => configuration.items.filter((endpoint) => endpoint.providerKind === selectedKind),
    [configuration.items, selectedKind],
  );

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) {
      return kindItems;
    }
    return kindItems.filter((endpoint) =>
      [
        endpoint.name,
        endpoint.baseUrl,
        endpoint.protocolDialect,
        endpoint.upstreamProtocolDialect ?? "",
      ]
        .join(" ")
        .toLowerCase()
        .includes(needle),
    );
  }, [kindItems, query]);

  function selectKind(kind: ProviderKind) {
    setQuery("");
    setExpandedIds(new Set());
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.set("kind", kind);
        next.delete("keys");
        next.delete("credential");
        next.delete("action");
        next.delete("editor");
        return next;
      },
      { replace: true },
    );
  }

  function isExpanded(id: string) {
    return expandedIds.has(id);
  }

  function clearCredentialParams(endpointId: string) {
    setSearchParams(
      (current) => {
        if (current.get("keys") !== endpointId) {
          return current;
        }
        const next = new URLSearchParams(current);
        next.delete("keys");
        next.delete("credential");
        next.delete("action");
        return next;
      },
      { replace: true },
    );
  }

  function openCreateCredential(endpointId: string) {
    // Open the drawer only — do not force accordion expansion.
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.delete("editor");
        next.delete("action");
        next.set("kind", selectedKind);
        next.set("keys", endpointId);
        next.set("credential", "new");
        return next;
      },
      { replace: true },
    );
  }

  function ensureExpanded(endpointId: string) {
    setExpandedIds((current) => {
      if (current.has(endpointId)) {
        return current;
      }
      const next = new Set(current);
      next.add(endpointId);
      return next;
    });
  }

  function toggleExpanded(id: string) {
    const open = isExpanded(id);
    setExpandedIds((current) => {
      const next = new Set(current);
      if (open) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
    if (open) {
      clearCredentialParams(id);
    }
  }

  const kindName = providerKindLabel(selectedKind);

  return (
    /*
     * Desktop grid:
     *   row1 col2 = right-only toolbar (does not sit above kinds)
     *   row2 col1 = kinds, row2 col2 = list  → first kind top == first list top
     * Mobile: stack toolbar → kinds → list.
     */
    <div className="grid grid-cols-1 gap-x-5 gap-y-3 sm:grid-cols-[13rem_minmax(0,1fr)] lg:grid-cols-[14rem_minmax(0,1fr)]">
      <div className="flex flex-col gap-2.5 sm:col-start-2 sm:row-start-1 sm:flex-row sm:items-center sm:justify-between">
        <div className="relative min-w-0 flex-1 sm:max-w-sm">
          <Search
            size={14}
            className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-tertiary"
            aria-hidden="true"
          />
          <input
            className="focus-ring h-8 w-full rounded-[8px] border-0 bg-surface-muted py-0 pl-8 pr-3 text-[12px] text-primary placeholder:text-tertiary"
            value={query}
            placeholder={`搜索 ${kindName} Endpoint`}
            aria-label={`搜索 ${kindName}`}
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <Button variant="ghost" disabled={refreshing} onClick={onRefresh}>
            <RefreshCw size={14} className={refreshing ? "animate-spin" : undefined} />
            刷新
          </Button>
          <Button variant="primary" disabled={pending} onClick={() => onCreate(selectedKind)}>
            <Plus size={14} />
            新增
          </Button>
        </div>
      </div>

      <div className="sm:col-start-1 sm:row-start-2">
        <ProviderKindNav selected={selectedKind} counts={counts} onSelect={selectKind} />
      </div>

      <div className="min-w-0 sm:col-start-2 sm:row-start-2">
        {filtered.length === 0 ? (
          <Surface className="flex min-h-48 flex-col items-center justify-center px-4 py-10 text-center">
            <p className="text-[13px] font-medium">
              {kindItems.length === 0
                ? `还没有 ${kindName} Endpoint`
                : "没有匹配的 Endpoint"}
            </p>
            <p className="mt-1 text-[12px] text-secondary">
              {kindItems.length === 0
                ? `添加 ${kindName} 上游地址。`
                : "试试其他关键词。"}
            </p>
          </Surface>
        ) : (
          <div className="space-y-2.5">
            {filtered.map((endpoint) => {
              const expanded = isExpanded(endpoint.id);
              const activeForKeys = activeKeysEndpoint === endpoint.id;
              const mountCredentials = expanded || activeForKeys;
              const panelId = `endpoint-keys-${endpoint.id}`;
              return (
                <Surface
                  key={endpoint.id}
                  className={cn("overflow-hidden transition-shadow", expanded && "shadow-sm")}
                  aria-label={endpoint.name}
                >
                  <div className="px-2.5 py-2 sm:px-3">
                    <ProviderEndpointTableRow
                      endpoint={endpoint}
                      pending={pending}
                      expanded={expanded}
                      onToggle={() => toggleExpanded(endpoint.id)}
                      onEdit={onEdit}
                      onCreateCredential={openCreateCredential}
                      onDelete={onDelete}
                    />
                  </div>
                  {mountCredentials ? (
                    <div
                      id={panelId}
                      className={expanded ? "border-t border-subtle/80" : undefined}
                      role={expanded ? "region" : undefined}
                      aria-label={expanded ? `${endpoint.name} 的 API Key` : undefined}
                    >
                      <div
                        className={
                          expanded
                            ? cn(
                                ENDPOINT_CONTENT_GRID_CLASS,
                                "px-2.5 pb-2 pt-0.5 sm:px-3",
                              )
                            : undefined
                        }
                      >
                        {expanded ? <div aria-hidden="true" /> : null}
                        <div className={expanded ? "min-w-0" : undefined}>
                          <ProviderCredentialManagement
                            endpoint={endpoint}
                            embedded
                            showList={expanded}
                            onRevealList={() => ensureExpanded(endpoint.id)}
                          />
                        </div>
                      </div>
                    </div>
                  ) : null}
                </Surface>
              );
            })}
          </div>
        )}

        <div className="flex flex-wrap items-center justify-between gap-2 pt-3 text-[12px] text-secondary">
          <p>
            {kindName} · 配置版本{" "}
            <span className="font-medium tabular-nums text-primary">
              {configuration.configRevision}
            </span>
            {" · "}
            共 <span className="tabular-nums">{filtered.length}</span> 条
          </p>
        </div>

        {actionError ? (
          <p className="pt-2 text-sm text-danger" role="alert">
            {getProviderErrorMessage(actionError)}
          </p>
        ) : null}
      </div>
    </div>
  );
}

function resolveSelectedKind(value: string | null): ProviderKind {
  if (isProviderKind(value)) {
    return value;
  }
  return PROVIDER_KIND_OPTIONS[0]?.kind ?? "codex";
}
