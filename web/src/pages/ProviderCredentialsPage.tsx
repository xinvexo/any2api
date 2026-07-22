import { ArrowLeft, RefreshCw } from "lucide-react";
import { Link, useParams } from "react-router-dom";

import { ProviderCredentialManagement, useProviderEndpoints } from "@/features/providers";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function ProviderCredentialsPage() {
  const { endpointId = "" } = useParams();
  const endpoints = useProviderEndpoints();
  const endpoint = endpoints.data?.items.find((item) => item.id === endpointId);

  if (endpoints.isPending && !endpoints.data) {
    return (
      <div className="flex min-h-56 items-center justify-center text-sm text-secondary">
        正在读取 Provider
      </div>
    );
  }

  if (!endpoint) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">Provider Endpoint 不存在</p>
        <p className="mt-2 text-sm text-secondary">该地址可能已被删除，或链接已经失效。</p>
        <div className="mt-5 flex flex-wrap gap-2">
          <Link
            className="focus-ring inline-flex h-8 items-center gap-1.5 rounded-[8px] px-3 text-[12px] font-medium text-secondary hover:bg-surface-muted hover:text-primary"
            to="/providers"
          >
            <ArrowLeft size={14} />
            返回 Provider
          </Link>
          <Button onClick={() => void endpoints.refetch()} disabled={endpoints.isFetching}>
            <RefreshCw size={14} className={endpoints.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
        </div>
      </Surface>
    );
  }

  return (
    <div>
      <div className="mb-4 flex flex-wrap items-center gap-3 border-b border-subtle pb-3">
        <Link
          className="focus-ring inline-flex h-8 items-center gap-1.5 rounded-[8px] px-2 text-[12px] font-medium text-secondary hover:bg-surface-muted hover:text-primary"
          to="/providers"
        >
          <ArrowLeft size={14} />
          Provider
        </Link>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="rounded-md bg-surface-muted px-1.5 py-0.5 text-[11px] font-medium uppercase text-secondary">
              {endpoint.providerKind}
            </span>
            <h1 className="truncate text-[13px] font-semibold">{endpoint.name}</h1>
          </div>
          <p className="mt-0.5 break-all text-[12px] text-tertiary">{endpoint.baseUrl}</p>
        </div>
      </div>
      <ProviderCredentialManagement endpoint={endpoint} />
    </div>
  );
}
