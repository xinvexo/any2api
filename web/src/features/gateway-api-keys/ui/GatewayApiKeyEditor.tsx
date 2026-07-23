import { Save } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { getGatewayApiKeyErrorMessage } from "../model/gateway-api-key-error";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { Field, FormError } from "@/shared/ui/form-field";
import { Switch } from "@/shared/ui/Switch";

export interface GatewayApiKeyEditorSubmit {
  name: string;
  enabled: boolean;
  regenerateToken: boolean;
}

interface GatewayApiKeyEditorProps {
  apiKey?: GatewayApiKey;
  pending: boolean;
  error: unknown;
  onSubmit: (input: GatewayApiKeyEditorSubmit) => Promise<void>;
  onClose: () => void;
}

export function GatewayApiKeyEditor({
  apiKey,
  pending,
  error,
  onSubmit,
  onClose,
}: GatewayApiKeyEditorProps) {
  const [name, setName] = useState(apiKey?.name ?? "");
  const [enabled, setEnabled] = useState(apiKey?.enabled ?? true);
  const [regenerateToken, setRegenerateToken] = useState(false);
  const [validation, setValidation] = useState<string | null>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  const isEdit = Boolean(apiKey);

  useEffect(() => {
    nameRef.current?.focus();
  }, []);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!name.trim() || name.trim() !== name) {
      setValidation("名称不能为空，且首尾不能包含空格。");
      nameRef.current?.focus();
      return;
    }
    setValidation(null);
    try {
      await onSubmit({
        name,
        enabled,
        regenerateToken: isEdit ? regenerateToken : false,
      });
    } catch {
      // Mutation state renders the structured server error without discarding the draft.
    }
  }

  return (
    <form className="space-y-5" onSubmit={(event) => void submit(event)} noValidate>
      <Field label="名称" error={validation ?? undefined} htmlFor="gateway-key-name">
        <input
          id="gateway-key-name"
          ref={nameRef}
          className={controlClass(Boolean(validation))}
          value={name}
          maxLength={100}
          autoComplete="off"
          disabled={pending}
          aria-invalid={Boolean(validation)}
          aria-describedby={validation ? "gateway-key-name-error" : undefined}
          onChange={(event) => {
            setName(event.target.value);
            if (validation) {
              setValidation(null);
            }
          }}
        />
      </Field>

      {isEdit ? (
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <p id="gateway-key-regenerate-label" className="text-[13px] font-medium">
              重新生成密钥
            </p>
            <p className="mt-0.5 text-[12px] text-secondary">开启后保存会生成新密钥，旧密钥立即失效</p>
          </div>
          <Switch
            id="gateway-key-regenerate"
            checked={regenerateToken}
            disabled={pending}
            aria-labelledby="gateway-key-regenerate-label"
            onCheckedChange={setRegenerateToken}
          />
        </div>
      ) : null}

      <div className="flex items-center justify-between gap-4">
        <p id="gateway-key-enabled-label" className="text-[13px] font-medium">
          启用此密钥
        </p>
        <Switch
          id="gateway-key-enabled"
          checked={enabled}
          disabled={pending}
          aria-labelledby="gateway-key-enabled-label"
          onCheckedChange={setEnabled}
        />
      </div>

      <FormError>{error ? getGatewayApiKeyErrorMessage(error) : null}</FormError>

      <div className="flex flex-col-reverse gap-2 border-t border-subtle pt-4 sm:flex-row sm:justify-end">
        <Button type="button" variant="ghost" disabled={pending} onClick={onClose}>
          取消
        </Button>
        <Button type="submit" variant="primary" disabled={pending}>
          <Save size={15} />
          {pending ? "正在保存" : "保存"}
        </Button>
      </div>
    </form>
  );
}
