import { KeyRound, Pencil, RefreshCw, ShieldOff } from "lucide-react";

import type { GatewayApiKey, GatewayApiKeyConfiguration } from "../api/gateway-api-key-contracts";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function GatewayApiKeyList({
  configuration,
  pending,
  onEdit,
  onRotate,
  onRevoke,
}: {
  configuration: GatewayApiKeyConfiguration;
  pending: boolean;
  onEdit: (id: string) => void;
  onRotate: (key: GatewayApiKey) => void;
  onRevoke: (key: GatewayApiKey) => void;
}) {
  if (configuration.items.length === 0) {
    return (
      <Surface className="flex min-h-56 items-center justify-center p-7 text-center">
        <div>
          <KeyRound size={23} className="mx-auto text-tertiary" aria-hidden="true" />
          <p className="mt-3 text-sm font-medium">尚未创建网关密钥</p>
        </div>
      </Surface>
    );
  }

  return (
    <Surface className="divide-y divide-subtle overflow-hidden">
      {configuration.items.map((key) => (
        <article key={key.id} className="p-5 sm:p-6">
          <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
                <h2 className="break-words font-semibold [overflow-wrap:anywhere]">{key.name}</h2>
                <Status keyValue={key} />
              </div>
              <p className="mt-2 font-mono text-sm text-secondary">{key.tokenPrefix}...</p>
              <dl className="mt-4 grid gap-x-6 gap-y-2 text-sm sm:grid-cols-2">
                <Field label="创建时间" value={formatTimestamp(key.createdAt)} />
                <Field label="最后使用" value={key.lastUsedAt ? formatTimestamp(key.lastUsedAt) : "尚无记录"} />
                <Field label="Token 版本" value={String(key.tokenVersion)} />
                <Field label="配置版本" value={String(key.configVersion)} />
              </dl>
            </div>
            <div className="flex flex-wrap gap-2 xl:max-w-72 xl:justify-end">
              <Button onClick={() => onEdit(key.id)} disabled={pending || key.revokedAt !== null}>
                <Pencil size={15} />
                编辑
              </Button>
              <Button onClick={() => onRotate(key)} disabled={pending || key.revokedAt !== null}>
                <RefreshCw size={15} />
                轮换
              </Button>
              <Button
                variant="danger"
                onClick={() => onRevoke(key)}
                disabled={pending || key.revokedAt !== null}
              >
                <ShieldOff size={15} />
                撤销
              </Button>
            </div>
          </div>
        </article>
      ))}
    </Surface>
  );
}

function Status({ keyValue }: { keyValue: GatewayApiKey }) {
  const value = keyValue.revokedAt ? "已撤销" : keyValue.enabled ? "已启用" : "已停用";
  const color = keyValue.revokedAt ? "bg-danger" : keyValue.enabled ? "bg-success" : "bg-warning";
  return (
    <span className="inline-flex items-center gap-2 text-sm text-secondary">
      <span className={`size-2 rounded-full ${color}`} aria-hidden="true" />
      {value}
    </span>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex min-w-0 justify-between gap-3 sm:block">
      <dt className="text-secondary">{label}</dt>
      <dd className="truncate font-medium tabular-nums sm:mt-1">{value}</dd>
    </div>
  );
}

function formatTimestamp(value: string) {
  return value.replace(" ", " · ");
}
