import { Check, Copy, Eye, EyeOff, X } from "lucide-react";
import { useState } from "react";

import { Button } from "@/shared/ui/Button";

export function CredentialSecretReceipt({
  label,
  apiKey,
  onClose,
}: {
  label: string;
  apiKey: string;
  onClose: () => void;
}) {
  const [revealed, setRevealed] = useState(false);
  const [copied, setCopied] = useState(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(apiKey);
      setCopied(true);
    } catch {
      setRevealed(true);
    }
  }

  return (
    <div className="rounded-[10px] border border-accent/25 bg-accent/5 px-3 py-2.5" role="status">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-[12px] font-medium text-primary">
            「{label}」已保存
          </p>
          <p className="mt-0.5 text-[11px] text-secondary">
            请立即复制 API Key，关闭后无法再次查看明文。
          </p>
        </div>
        <Button
          variant="ghost"
          className="size-7 shrink-0 px-0"
          aria-label="关闭回执"
          onClick={onClose}
        >
          <X size={14} />
        </Button>
      </div>
      <div className="mt-2 flex flex-col gap-2 sm:flex-row sm:items-center">
        <input
          className="focus-ring h-8 min-w-0 flex-1 rounded-[8px] border-0 bg-surface px-2.5 font-mono text-[12px]"
          aria-label="本次保存的 API Key"
          type={revealed ? "text" : "password"}
          value={apiKey}
          readOnly
          spellCheck={false}
        />
        <div className="flex gap-1">
          <Button
            variant="ghost"
            className="size-8 px-0"
            aria-label={revealed ? "隐藏 API Key" : "显示 API Key"}
            onClick={() => setRevealed((value) => !value)}
          >
            {revealed ? <EyeOff size={14} /> : <Eye size={14} />}
          </Button>
          <Button variant="ghost" onClick={() => void copy()}>
            {copied ? <Check size={14} /> : <Copy size={14} />}
            {copied ? "已复制" : "复制"}
          </Button>
        </div>
      </div>
    </div>
  );
}
