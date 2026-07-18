import { Check, KeyRound, Pencil, RotateCw, Trash2, X } from "lucide-react";
import { useState } from "react";

import type {
  ProviderCredential,
  ProviderCredentialConfiguration,
} from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import type { ProxyConfiguration } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

interface ProviderCredentialListProps {
  configuration: ProviderCredentialConfiguration;
  proxies: ProxyConfiguration;
  pending: boolean;
  actionError: unknown;
  onEdit: (id: string) => void;
  onRotate: (id: string) => void;
  onDelete: (credential: ProviderCredential) => void;
}

export function ProviderCredentialList({
  configuration,
  proxies,
  pending,
  actionError,
  onEdit,
  onRotate,
  onDelete,
}: ProviderCredentialListProps) {
  return (
    <Surface className="overflow-hidden">
      <div className="flex items-center justify-between border-b border-subtle px-5 py-4 sm:px-6">
        <div>
          <h2 className="text-base font-semibold">API Key</h2>
          <p className="mt-1 text-sm text-secondary">独立代理、并发上限与启停状态</p>
        </div>
        <span className="text-sm tabular-nums text-tertiary">{configuration.items.length}</span>
      </div>
      {configuration.items.length > 0 ? (
        <ul className="divide-y divide-subtle">
          {configuration.items.map((credential) => (
            <CredentialRow
              key={credential.id}
              credential={credential}
              proxies={proxies}
              pending={pending}
              onEdit={onEdit}
              onRotate={onRotate}
              onDelete={onDelete}
            />
          ))}
        </ul>
      ) : (
        <div className="p-7 text-center">
          <KeyRound size={23} className="mx-auto text-tertiary" aria-hidden="true" />
          <p className="mt-3 text-sm font-medium">还没有 API Key</p>
          <p className="mt-1 text-sm text-secondary">添加后即可为这个 Endpoint 提供独立凭据。</p>
        </div>
      )}
      {actionError ? (
        <p className="border-t border-subtle px-5 py-3 text-sm text-danger sm:px-6" role="alert">
          {getProviderErrorMessage(actionError)}
        </p>
      ) : null}
    </Surface>
  );
}

function CredentialRow({
  credential,
  proxies,
  pending,
  onEdit,
  onRotate,
  onDelete,
}: {
  credential: ProviderCredential;
  proxies: ProxyConfiguration;
  pending: boolean;
  onEdit: (id: string) => void;
  onRotate: (id: string) => void;
  onDelete: (credential: ProviderCredential) => void;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const proxy = proxies.items.find((item) => item.id === credential.proxyProfileId);
  const proxyLabel = describeProxy(proxy?.id, proxies);

  return (
    <li className="px-5 py-5 sm:px-6">
      <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <p className="break-words font-semibold [overflow-wrap:anywhere]">{credential.label}</p>
            <Badge icon={credential.enabled ? <Check size={12} /> : <X size={12} />}>
              {credential.enabled ? "启用" : "已停用"}
            </Badge>
            <Badge>并发 {credential.maxConcurrency}</Badge>
          </div>
          <p className="mt-2 break-words text-sm text-secondary [overflow-wrap:anywhere]">
            {proxyLabel}
          </p>
          <p className="mt-1 font-mono text-xs text-tertiary">
            {credential.secretTail ? `•••• ${credential.secretTail}` : credential.fingerprint}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="ghost"
            disabled={pending}
            aria-label={`编辑 ${credential.label}`}
            onClick={() => onEdit(credential.id)}
          >
            <Pencil size={15} />
            编辑
          </Button>
          <Button
            variant="ghost"
            disabled={pending}
            aria-label={`轮换 ${credential.label}`}
            onClick={() => onRotate(credential.id)}
          >
            <RotateCw size={15} />
            轮换
          </Button>
          {confirmDelete ? (
            <>
              <Button
                variant="danger"
                disabled={pending}
                aria-label={`确认删除 ${credential.label}`}
                onClick={() => onDelete(credential)}
              >
                <Trash2 size={15} />
                确认删除
              </Button>
              <Button
                variant="ghost"
                disabled={pending}
                aria-label={`取消删除 ${credential.label}`}
                onClick={() => setConfirmDelete(false)}
              >
                <X size={15} />
                取消
              </Button>
            </>
          ) : (
            <Button
              variant="ghost"
              disabled={pending}
              aria-label={`删除 ${credential.label}`}
              onClick={() => setConfirmDelete(true)}
            >
              <Trash2 size={15} />
              删除
            </Button>
          )}
        </div>
      </div>
    </li>
  );
}

function describeProxy(proxyId: string | undefined, configuration: ProxyConfiguration) {
  const proxy = configuration.items.find((item) => item.id === proxyId);
  if (!proxy) {
    return "代理配置不存在";
  }
  if (proxy.kind !== "direct") {
    return `${proxy.name} · ${proxy.kind.toUpperCase()}${proxy.enabled ? "" : " · 已停用"}`;
  }
  const global = configuration.items.find((item) => item.id === configuration.globalProxyId);
  return global?.kind === "direct"
    ? "DIRECT · 本机直连"
    : `DIRECT · 继承全局 ${global?.name ?? "未知代理"}`;
}

function Badge({ children, icon }: { children: React.ReactNode; icon?: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-surface-muted px-2 py-1 text-[11px] font-semibold text-secondary">
      {icon}
      {children}
    </span>
  );
}
