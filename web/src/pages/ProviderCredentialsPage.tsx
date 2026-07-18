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
      <Surface className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary">
        正在读取 Provider
      </Surface>
    );
  }

  if (!endpoint) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">Provider Endpoint 不存在</p>
        <p className="mt-2 text-sm text-secondary">该地址可能已被删除，或链接已经失效。</p>
        <div className="mt-5 flex flex-wrap gap-2">
          <Link className={linkButtonClass} to="/providers">
            <ArrowLeft size={15} />
            返回 Provider
          </Link>
          <Button onClick={() => void endpoints.refetch()} disabled={endpoints.isFetching}>
            <RefreshCw size={15} className={endpoints.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
        </div>
      </Surface>
    );
  }

  return (
    <div className="space-y-7">
      <header>
        <Link className="focus-ring inline-flex items-center gap-2 rounded-control text-sm text-secondary hover:text-primary" to="/providers">
          <ArrowLeft size={15} />
          Provider
        </Link>
        <p className="mt-5 text-sm font-medium text-accent-copy">{endpoint.providerKind.toUpperCase()}</p>
        <h1 className="mt-2 break-words text-3xl font-semibold [overflow-wrap:anywhere] sm:text-[34px]">
          {endpoint.name}
        </h1>
        <p className="mt-3 break-all text-sm leading-6 text-secondary">{endpoint.baseUrl}</p>
      </header>
      <ProviderCredentialManagement endpoint={endpoint} />
    </div>
  );
}

const linkButtonClass =
  "focus-ring inline-flex h-10 items-center justify-center gap-2 rounded-control border border-subtle bg-surface px-4 text-sm font-semibold text-primary shadow-hairline transition-colors hover:bg-surface-hover";
