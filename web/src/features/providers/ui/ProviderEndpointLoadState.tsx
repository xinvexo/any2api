import { RefreshCw } from "lucide-react";

import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export type ProviderEndpointLoadState =
  | { kind: "loading" }
  | {
      kind: "error";
      message: string;
      refreshing: boolean;
      onRetry: () => void;
    };

export function ProviderEndpointLoadStateView({ state }: { state: ProviderEndpointLoadState }) {
  if (state.kind === "loading") {
    return (
      <div className="flex h-full min-h-48 items-center justify-center text-sm text-secondary">
        正在读取 Provider 配置
      </div>
    );
  }

  return (
    <Surface className="p-6" role="alert">
      <p className="font-semibold">无法读取 Provider 配置</p>
      <p className="mt-2 text-sm text-secondary">{state.message}</p>
      <Button className="mt-5" onClick={state.onRetry} disabled={state.refreshing}>
        <RefreshCw size={14} className={state.refreshing ? "animate-spin" : undefined} />
        重试
      </Button>
    </Surface>
  );
}
