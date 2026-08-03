import { Plus, RefreshCw, Search } from "lucide-react";
import type { ReactNode } from "react";

import type { ProviderKind } from "../api/provider-contracts";
import { providerKindLabel } from "../model/provider-kind-catalog";
import { ProviderKindNav } from "./ProviderKindNav";
import { Button } from "@/shared/ui/Button";
import { KindSplitLayout } from "@/shared/ui/KindSplitLayout";

export function ProviderEndpointChrome({
  selectedKind,
  counts,
  onSelectKind,
  search,
  busy = false,
  refreshing,
  onRefresh,
  createDisabled,
  onCreate,
  children,
}: {
  selectedKind: ProviderKind;
  counts: Record<ProviderKind, number>;
  onSelectKind: (kind: ProviderKind) => void;
  search?: { value: string; onChange: (value: string) => void };
  busy?: boolean;
  refreshing: boolean;
  onRefresh: () => void;
  createDisabled: boolean;
  onCreate: () => void;
  children: ReactNode;
}) {
  const kindName = providerKindLabel(selectedKind);
  return (
    <KindSplitLayout
      aria-busy={busy || undefined}
      toolbarStart={
        search ? (
          <>
            <Search
              size={14}
              className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-tertiary"
              aria-hidden="true"
            />
            <input
              className="focus-ring h-8 w-full rounded-[8px] border-0 bg-surface-muted py-0 pl-8 pr-3 text-[12px] text-primary placeholder:text-tertiary"
              value={search.value}
              placeholder={`搜索 ${kindName} Endpoint`}
              aria-label={`搜索 ${kindName}`}
              onChange={(event) => search.onChange(event.target.value)}
            />
          </>
        ) : undefined
      }
      toolbarEnd={
        <>
          <Button variant="ghost" disabled={refreshing || busy} onClick={onRefresh}>
            <RefreshCw size={14} className={refreshing ? "animate-spin" : undefined} />
            刷新
          </Button>
          <Button variant="primary" disabled={createDisabled || busy} onClick={onCreate}>
            <Plus size={14} />
            新增
          </Button>
        </>
      }
      kindNav={
        <ProviderKindNav selected={selectedKind} counts={counts} onSelect={onSelectKind} />
      }
    >
      {children}
    </KindSplitLayout>
  );
}
