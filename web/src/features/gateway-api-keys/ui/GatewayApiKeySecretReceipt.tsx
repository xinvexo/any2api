import { Check, Copy, Eye, EyeOff, KeyRound, X } from "lucide-react";
import { useState } from "react";

import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function GatewayApiKeySecretReceipt({
  name,
  token,
  onClose,
}: {
  name: string;
  token: string;
  onClose: () => void;
}) {
  const [revealed, setRevealed] = useState(false);
  const [copied, setCopied] = useState(false);
  const [copyFailed, setCopyFailed] = useState(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(token);
      setCopied(true);
      setCopyFailed(false);
    } catch {
      setRevealed(true);
      setCopyFailed(true);
    }
  }

  return (
    <Surface className="overflow-hidden border-accent/30" role="status">
      <div className="flex items-start justify-between gap-4 border-b border-subtle px-5 py-4 sm:px-6">
        <div className="flex min-w-0 gap-3">
          <span className="grid size-9 shrink-0 place-items-center rounded-control bg-surface-muted text-accent-copy">
            <KeyRound size={17} aria-hidden="true" />
          </span>
          <div className="min-w-0">
            <p className="break-words font-semibold [overflow-wrap:anywhere]">{name}</p>
            <p className="mt-1 text-sm text-secondary">关闭后将无法再次查看完整密钥</p>
          </div>
        </div>
        <Button
          variant="ghost"
          className="size-9 shrink-0 px-0"
          aria-label="关闭密钥回执"
          onClick={onClose}
        >
          <X size={17} />
        </Button>
      </div>
      <div className="flex flex-col gap-3 p-5 sm:flex-row sm:items-center sm:p-6">
        <input
          className="focus-ring h-10 min-w-0 flex-1 rounded-control border border-subtle bg-surface px-3 font-mono text-sm"
          aria-label="本次生成的网关密钥"
          type={revealed ? "text" : "password"}
          value={token}
          readOnly
          spellCheck={false}
        />
        <div className="flex gap-2">
          <Button
            variant="ghost"
            className="size-10 px-0"
            aria-label={revealed ? "隐藏网关密钥" : "显示网关密钥"}
            onClick={() => setRevealed((value) => !value)}
          >
            {revealed ? <EyeOff size={16} /> : <Eye size={16} />}
          </Button>
          <Button onClick={() => void copy()}>
            {copied ? <Check size={15} /> : <Copy size={15} />}
            {copied ? "已复制" : "复制"}
          </Button>
        </div>
      </div>
      {copyFailed ? (
        <p className="border-t border-subtle px-5 py-3 text-sm text-warning sm:px-6">
          浏览器未允许复制，请显示后手动复制。
        </p>
      ) : null}
    </Surface>
  );
}
