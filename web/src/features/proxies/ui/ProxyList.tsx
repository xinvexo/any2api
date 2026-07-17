import { Check, Pencil, ShieldCheck, Trash2, X } from "lucide-react";
import { useState } from "react";

import type { ProxyConfiguration, ProxyProfile } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

interface ProxyListProps {
  configuration: ProxyConfiguration;
  pending: boolean;
  actionError: unknown;
  onEdit: (id: string) => void;
  onSetGlobal: (id: string) => void;
  onDelete: (id: string) => void;
}

export function ProxyList({
  configuration,
  pending,
  actionError,
  onEdit,
  onSetGlobal,
  onDelete,
}: ProxyListProps) {
  return (
    <Surface className="overflow-hidden">
      <div className="flex items-center justify-between border-b border-subtle px-5 py-4 sm:px-6">
        <div>
          <h2 className="text-base font-semibold">代理列表</h2>
          <p className="mt-1 text-sm text-secondary">DIRECT 固定置顶且始终可用</p>
        </div>
        <span className="text-sm tabular-nums text-tertiary">{configuration.items.length}</span>
      </div>
      <ul className="divide-y divide-subtle">
        {configuration.items.map((proxy) => (
          <ProxyRow
            key={proxy.id}
            proxy={proxy}
            isGlobal={proxy.id === configuration.globalProxyId}
            pending={pending}
            onEdit={onEdit}
            onSetGlobal={onSetGlobal}
            onDelete={onDelete}
          />
        ))}
      </ul>
      {configuration.items.length === 1 ? (
        <p className="border-t border-subtle px-5 py-4 text-sm text-secondary sm:px-6">
          尚未添加自定义代理。新代理会独立保存，不会改变当前全局出口。
        </p>
      ) : null}
      {actionError ? (
        <p className="border-t border-subtle px-5 py-3 text-sm text-danger sm:px-6" role="alert">
          {getProxyErrorMessage(actionError)}
        </p>
      ) : null}
    </Surface>
  );
}

interface ProxyRowProps {
  proxy: ProxyProfile;
  isGlobal: boolean;
  pending: boolean;
  onEdit: (id: string) => void;
  onSetGlobal: (id: string) => void;
  onDelete: (id: string) => void;
}

function ProxyRow({
  proxy,
  isGlobal,
  pending,
  onEdit,
  onSetGlobal,
  onDelete,
}: ProxyRowProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const endpoint = proxy.host && proxy.port ? `${proxy.host}:${proxy.port}` : "本机网络";

  return (
    <li className="px-5 py-5 sm:px-6">
      <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <p className="min-w-0 break-words font-semibold [overflow-wrap:anywhere]">{proxy.name}</p>
            <Badge>{proxy.kind.toUpperCase()}</Badge>
            {proxy.builtIn ? <Badge icon={<ShieldCheck size={12} />}>内置</Badge> : null}
            {isGlobal ? <Badge icon={<Check size={12} />}>全局</Badge> : null}
            {!proxy.enabled ? <Badge>已停用</Badge> : null}
          </div>
          <p className="mt-2 break-all text-sm text-secondary">{endpoint}</p>
          {proxy.builtIn ? (
            <p className="mt-1 text-xs text-tertiary">不可编辑、删除或停用</p>
          ) : null}
        </div>
        {!proxy.builtIn ? (
          <div className="flex flex-wrap gap-2">
            {!isGlobal && proxy.enabled ? (
              <Button
                variant="ghost"
                disabled={pending}
                aria-label={`设为全局 ${proxy.name}`}
                onClick={() => onSetGlobal(proxy.id)}
              >
                设为全局
              </Button>
            ) : null}
            <Button
              variant="ghost"
              disabled={pending}
              aria-label={`编辑 ${proxy.name}`}
              onClick={() => onEdit(proxy.id)}
            >
              <Pencil size={15} />
              编辑
            </Button>
            {confirmDelete ? (
              <>
                <Button
                  variant="danger"
                  disabled={pending || isGlobal}
                  aria-label={`确认删除 ${proxy.name}`}
                  onClick={() => onDelete(proxy.id)}
                >
                  <Trash2 size={15} />
                  确认删除
                </Button>
                <Button
                  variant="ghost"
                  disabled={pending}
                  aria-label={`取消删除 ${proxy.name}`}
                  onClick={() => setConfirmDelete(false)}
                >
                  <X size={15} />
                  取消
                </Button>
              </>
            ) : (
              <Button
                variant="ghost"
                disabled={pending || isGlobal}
                aria-label={`删除 ${proxy.name}`}
                onClick={() => setConfirmDelete(true)}
              >
                <Trash2 size={15} />
                删除
              </Button>
            )}
          </div>
        ) : null}
      </div>
    </li>
  );
}

function Badge({ children, icon }: { children: React.ReactNode; icon?: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-surface-muted px-2 py-1 text-[11px] font-semibold text-secondary">
      {icon}
      {children}
    </span>
  );
}
