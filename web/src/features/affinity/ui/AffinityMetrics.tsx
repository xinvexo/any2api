import { Link2, LoaderCircle, LockKeyhole, Trash2 } from "lucide-react";

import type { AffinityCredentialCount } from "../api/affinity-contracts";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export interface AffinityMetricsProps {
  soft: number;
  hard: number;
  creating: number;
  credentials: AffinityCredentialCount[];
  pendingCredentialId: string | null;
  onClearCredential: (credentialId: string) => void;
}

export function AffinityMetrics({
  soft,
  hard,
  creating,
  credentials,
  pendingCredentialId,
  onClearCredential,
}: AffinityMetricsProps) {
  return (
    <div className="space-y-5">
      <div className="grid gap-4 sm:grid-cols-3">
        <Metric icon={Link2} label="软绑定" value={soft} detail="普通会话可按策略重绑" />
        <Metric icon={LockKeyhole} label="硬绑定" value={hard} detail="Response ID 精确续接" />
        <Metric icon={LoaderCircle} label="正在创建" value={creating} detail="同会话并发只创建一次" />
      </div>

      <Surface className="overflow-hidden">
        <div className="border-b border-subtle px-5 py-4">
          <h2 className="font-semibold">Credential 绑定分布</h2>
          <p className="mt-1 text-sm text-secondary">固定等待只在同一个 Credential 内获得优先级。</p>
        </div>
        {credentials.length === 0 ? (
          <p className="px-5 py-8 text-center text-sm text-secondary">当前没有会话绑定</p>
        ) : (
          <div className="divide-y divide-subtle">
            {credentials.map((credential) => (
              <div
                key={credential.credentialId}
                className="flex flex-col gap-4 px-5 py-4 sm:flex-row sm:items-center"
              >
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-semibold" title={credential.credentialLabel}>
                    {credential.credentialLabel}
                  </p>
                  <p className="truncate font-mono text-sm" title={credential.credentialId}>
                    {credential.credentialId}
                  </p>
                  <p className="mt-1 text-sm text-secondary">
                    软绑定 {credential.softBindings} · 硬绑定 {credential.hardBindings}
                  </p>
                </div>
                <Button
                  variant="ghost"
                  onClick={() => onClearCredential(credential.credentialId)}
                  disabled={pendingCredentialId !== null}
                  aria-label={`清除 Credential ${credential.credentialId} 的会话绑定`}
                >
                  <Trash2 size={15} />
                  {pendingCredentialId === credential.credentialId ? "正在清除" : "清除绑定"}
                </Button>
              </div>
            ))}
          </div>
        )}
      </Surface>
    </div>
  );
}

function Metric({
  icon: Icon,
  label,
  value,
  detail,
}: {
  icon: typeof Link2;
  label: string;
  value: number;
  detail: string;
}) {
  return (
    <Surface className="p-5">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-secondary">{label}</p>
        <Icon size={17} className="text-tertiary" aria-hidden="true" />
      </div>
      <p className="mt-3 text-3xl font-semibold tabular-nums">{value}</p>
      <p className="mt-2 text-xs leading-5 text-tertiary">{detail}</p>
    </Surface>
  );
}
