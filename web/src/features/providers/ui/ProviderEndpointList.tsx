import { SearchX, Server } from "lucide-react";
import { useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";

import type {
  ProviderEndpoint,
  ProviderKind,
} from "../api/provider-contracts";
import {
  isProviderKind,
  providerKindLabel,
  PROVIDER_KIND_OPTIONS,
} from "../model/provider-kind-catalog";
import { ProviderCredentialManagement } from "./ProviderCredentialManagement";
import { ProviderEndpointChrome } from "./ProviderEndpointChrome";
import {
  ProviderEndpointLoadStateView,
  type ProviderEndpointLoadState,
} from "./ProviderEndpointLoadState";
import {
  ENDPOINT_CONTENT_GRID_CLASS,
  ProviderEndpointTableRow,
} from "./ProviderEndpointTableRow";
import { cn } from "@/shared/lib/cn";

interface ProviderEndpointListProps {
  items: readonly ProviderEndpoint[];
  loadState?: ProviderEndpointLoadState;
  pending: boolean;
  refreshing: boolean;
  onCreate: (kind: ProviderKind) => void;
  onRefresh: () => void;
  onEdit: (id: string) => void;
  onToggleEnabled: (endpoint: ProviderEndpoint) => void;
  onDelete: (endpoint: ProviderEndpoint) => void;
}

export function ProviderEndpointList({
  items,
  loadState,
  pending,
  refreshing,
  onCreate,
  onRefresh,
  onEdit,
  onToggleEnabled,
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
    for (const endpoint of items) {
      next[endpoint.providerKind] = (next[endpoint.providerKind] ?? 0) + 1;
    }
    return next;
  }, [items]);

  const kindItems = useMemo(
    () => items.filter((endpoint) => endpoint.providerKind === selectedKind),
    [items, selectedKind],
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
  const kindIsEmpty = kindItems.length === 0;
  const emptyLabel = kindIsEmpty
    ? `暂无 ${kindName} Endpoint`
    : `没有匹配的 ${kindName} Endpoint`;
  const EmptyIcon = kindIsEmpty ? Server : SearchX;

  return (
    <ProviderEndpointChrome
      selectedKind={selectedKind}
      counts={counts}
      onSelectKind={selectKind}
      search={loadState ? undefined : { value: query, onChange: setQuery }}
      busy={loadState?.kind === "loading"}
      refreshing={refreshing}
      onRefresh={onRefresh}
      createDisabled={pending || loadState !== undefined}
      onCreate={() => onCreate(selectedKind)}
    >
      {loadState ? (
        <ProviderEndpointLoadStateView state={loadState} />
      ) : (
        <div className="flex h-full min-h-0 flex-col">
        {filtered.length === 0 ? (
          <div
            className="flex min-h-40 flex-1 flex-col items-center justify-center px-6 py-9 text-center"
            role="status"
            aria-label={emptyLabel}
          >
            <EmptyIcon size={20} strokeWidth={1.6} className="text-tertiary" aria-hidden="true" />
            <p className="mt-2.5 text-[13px] font-medium text-primary">
              {kindIsEmpty ? `还没有 ${kindName} Endpoint` : "没有匹配的 Endpoint"}
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            {filtered.map((endpoint) => {
              const expanded = isExpanded(endpoint.id);
              const activeForKeys = activeKeysEndpoint === endpoint.id;
              const mountCredentials = expanded || activeForKeys;
              const panelId = `endpoint-keys-${endpoint.id}`;
              return (
                <section
                  key={endpoint.id}
                  aria-label={endpoint.name}
                  className={cn(
                    "min-w-0 overflow-hidden rounded-[14px] bg-surface-muted/45 transition-colors",
                    expanded && "bg-surface-muted/60",
                  )}
                >
                  <div className="min-w-0 px-2.5 py-2 sm:px-3">
                    <ProviderEndpointTableRow
                      endpoint={endpoint}
                      pending={pending}
                      expanded={expanded}
                      onToggle={() => toggleExpanded(endpoint.id)}
                      onEdit={onEdit}
                      onCreateCredential={openCreateCredential}
                      onToggleEnabled={onToggleEnabled}
                      onDelete={onDelete}
                    />
                  </div>
                  {mountCredentials ? (
                    <div
                      id={panelId}
                      className={cn("min-w-0", expanded && "bg-surface/45")}
                      role={expanded ? "region" : undefined}
                      aria-label={expanded ? `${endpoint.name} 的 API Key` : undefined}
                    >
                      {expanded ? (
                        <div className="mx-2.5 border-t border-subtle/40 sm:mx-3" />
                      ) : null}
                      {/* Indent keys under endpoint title, not under the chevron column. */}
                      <div
                        className={
                          expanded
                            ? cn(
                                ENDPOINT_CONTENT_GRID_CLASS,
                                "min-w-0 pb-2 pt-1.5 sm:px-3",
                              )
                            : undefined
                        }
                      >
                        {expanded ? <div className="hidden sm:block" aria-hidden="true" /> : null}
                        <div
                          className={expanded ? "col-span-2 min-w-0 overflow-hidden sm:col-span-1" : undefined}
                        >
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
                </section>
              );
            })}
          </div>
        )}

        <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 pt-3 text-[12px] text-secondary">
          <p>
            {kindName} · 共 <span className="tabular-nums">{filtered.length}</span> 条
          </p>
        </div>
        </div>
      )}
    </ProviderEndpointChrome>
  );
}

function resolveSelectedKind(value: string | null): ProviderKind {
  if (isProviderKind(value)) {
    return value;
  }
  return PROVIDER_KIND_OPTIONS[0]?.kind ?? "openai";
}
