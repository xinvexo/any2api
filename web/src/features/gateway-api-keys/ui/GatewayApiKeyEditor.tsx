import { KeyRound, X } from "lucide-react";
import { FormEvent, useState } from "react";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { getGatewayApiKeyErrorMessage } from "../model/gateway-api-key-error";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function GatewayApiKeyEditor({
  apiKey,
  configRevision,
  pending,
  error,
  onSubmit,
  onClose,
}: {
  apiKey?: GatewayApiKey;
  configRevision: number;
  pending: boolean;
  error: unknown;
  onSubmit: (input: { name: string; enabled: boolean }) => Promise<void>;
  onClose: () => void;
}) {
  const [name, setName] = useState(apiKey?.name ?? "");
  const [enabled, setEnabled] = useState(apiKey?.enabled ?? true);
  const [validation, setValidation] = useState<string | null>(null);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!name.trim() || name.trim() !== name) {
      setValidation("名称不能为空，且首尾不能包含空格。");
      return;
    }
    setValidation(null);
    await onSubmit({ name, enabled });
  }

  return (
    <Surface className="lg:sticky lg:top-24">
      <form onSubmit={(event) => void submit(event)}>
        <div className="flex items-start justify-between gap-4 border-b border-subtle px-5 py-4 sm:px-6">
          <div className="flex min-w-0 gap-3">
            <span className="grid size-9 shrink-0 place-items-center rounded-control bg-surface-muted text-accent-copy">
              <KeyRound size={17} aria-hidden="true" />
            </span>
            <div>
              <h2 className="font-semibold">{apiKey ? "编辑网关密钥" : "新增网关密钥"}</h2>
              <p className="mt-1 text-sm text-secondary">配置版本 {configRevision}</p>
            </div>
          </div>
          <Button variant="ghost" className="size-9 px-0" aria-label="关闭编辑器" onClick={onClose}>
            <X size={17} />
          </Button>
        </div>
        <div className="space-y-5 p-5 sm:p-6">
          <label className="block text-sm font-medium">
            名称
            <input
              className="focus-ring mt-2 h-8 w-full rounded-control border border-subtle bg-surface px-2.5 text-[12px]"
              value={name}
              onChange={(event) => setName(event.target.value)}
              maxLength={100}
              disabled={pending}
            />
          </label>
          <label className="flex items-center gap-3 text-sm font-medium">
            <input
              className="size-4 accent-accent"
              type="checkbox"
              checked={enabled}
              onChange={(event) => setEnabled(event.target.checked)}
              disabled={pending}
            />
            启用此密钥
          </label>
          {validation || error ? (
            <p className="text-sm text-danger" role="alert">
              {validation ?? getGatewayApiKeyErrorMessage(error)}
            </p>
          ) : null}
        </div>
        <div className="flex justify-end gap-2 border-t border-subtle px-5 py-4 sm:px-6">
          <Button onClick={onClose} disabled={pending}>取消</Button>
          <Button type="submit" variant="primary" disabled={pending}>
            {pending ? "保存中" : "保存"}
          </Button>
        </div>
      </form>
    </Surface>
  );
}
