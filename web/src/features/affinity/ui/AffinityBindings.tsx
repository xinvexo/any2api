import type { AffinityBinding } from "../api/affinity-contracts";
import { Surface } from "@/shared/ui/Surface";

export function AffinityBindings({ bindings }: { bindings: AffinityBinding[] }) {
  return (
    <Surface className="overflow-hidden">
      <div className="border-b border-subtle px-5 py-4">
        <h2 className="font-semibold">脱敏绑定样本</h2>
        <p className="mt-1 text-sm text-secondary">
          会话与 Response ID 仅显示进程内 HMAC 的短前缀，原始值不会进入管理接口。
        </p>
      </div>
      {bindings.length === 0 ? (
        <p className="px-5 py-8 text-center text-sm text-secondary">暂无可展示的绑定样本</p>
      ) : (
        <div className="divide-y divide-subtle">
          {bindings.map((binding) => (
            <article
              key={`${binding.kind}:${binding.sessionHashPrefix}`}
              className="grid gap-3 px-5 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-center"
            >
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="rounded-full bg-surface-muted px-2.5 py-1 text-xs font-semibold">
                    {binding.kind === "hard" ? "硬绑定" : "软绑定"}
                  </span>
                  <code className="text-xs text-secondary">{binding.sessionHashPrefix}</code>
                </div>
                <p className="mt-2 truncate text-sm font-medium" title={binding.upstreamModel}>
                  {binding.upstreamModel}
                </p>
                <p className="mt-1 truncate font-mono text-xs text-tertiary" title={binding.credentialId}>
                  {binding.credentialId}
                </p>
              </div>
              <div className="text-left text-xs text-secondary md:text-right">
                <p>{dialectLabel(binding.protocolDialect)}</p>
                <p className="mt-1 tabular-nums">剩余 {formatDuration(binding.expiresInMs)}</p>
              </div>
            </article>
          ))}
        </div>
      )}
    </Surface>
  );
}

function dialectLabel(dialect: AffinityBinding["protocolDialect"]) {
  switch (dialect) {
    case "openai_responses":
      return "Codex Responses";
    case "anthropic_messages":
      return "Claude Messages";
    case "codex_backend":
      return "Codex Backend";
  }
}

function formatDuration(milliseconds: number) {
  if (milliseconds < 60_000) {
    return `${Math.ceil(milliseconds / 1_000)} 秒`;
  }
  if (milliseconds < 3_600_000) {
    return `${Math.ceil(milliseconds / 60_000)} 分钟`;
  }
  return `${Math.ceil(milliseconds / 3_600_000)} 小时`;
}
