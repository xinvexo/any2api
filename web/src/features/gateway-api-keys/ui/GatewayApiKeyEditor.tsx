import { RefreshCw } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { getGatewayApiKeyErrorMessage } from "../model/gateway-api-key-error";
import {
  generateGatewayApiKeyToken,
  isGatewayApiKeyToken,
} from "../model/generate-gateway-api-key-token";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { Field, FormError } from "@/shared/ui/form-field";
import { Switch } from "@/shared/ui/Switch";

export interface GatewayApiKeyEditorSubmit {
  name: string;
  enabled: boolean;
  token: string;
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
  const isEdit = Boolean(apiKey);
  const [name, setName] = useState(apiKey?.name ?? "");
  const [enabled, setEnabled] = useState(apiKey?.enabled ?? true);
  const [token, setToken] = useState(() => apiKey?.token ?? generateGatewayApiKeyToken());
  const [validation, setValidation] = useState<string | null>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  const tokenRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    nameRef.current?.focus();
  }, []);

  function generateToken() {
    const next = generateGatewayApiKeyToken();
    setToken(next);
    setValidation(null);
    // Keep the generated value visible and selectable for copy.
    window.requestAnimationFrame(() => {
      tokenRef.current?.focus();
      tokenRef.current?.select();
    });
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!name.trim() || name.trim() !== name) {
      setValidation("名称不能为空，且首尾不能包含空格。");
      nameRef.current?.focus();
      return;
    }
    const nextToken = token.trim();
    if (!isGatewayApiKeyToken(nextToken)) {
      setValidation("密钥必须以 sk- 开头，后接 48 位大小写字母或数字。");
      tokenRef.current?.focus();
      return;
    }
    setValidation(null);
    try {
      await onSubmit({
        name,
        enabled,
        token: nextToken,
      });
    } catch {
      // Mutation state renders the structured server error without discarding the draft.
    }
  }

  return (
    <form className="space-y-5" onSubmit={(event) => void submit(event)} noValidate>
      <Field label="名称" error={validation?.includes("名称") ? validation : undefined} htmlFor="gateway-key-name">
        <input
          id="gateway-key-name"
          ref={nameRef}
          className={controlClass(Boolean(validation?.includes("名称")))}
          value={name}
          maxLength={100}
          autoComplete="off"
          disabled={pending}
          aria-invalid={Boolean(validation?.includes("名称"))}
          onChange={(event) => {
            setName(event.target.value);
            if (validation) {
              setValidation(null);
            }
          }}
        />
      </Field>

      <Field
        label="密钥"
        error={validation?.includes("密钥") ? validation : undefined}
        htmlFor="gateway-key-token"
        hint={
          isEdit
            ? "可直接修改，或点生成替换为新密钥；保存后旧密钥立即失效。"
            : "默认已自动生成，可手动修改；格式为 sk- 加 48 位字母数字。"
        }
      >
        <div className="flex items-center gap-2">
          <input
            id="gateway-key-token"
            ref={tokenRef}
            className={controlClass(Boolean(validation?.includes("密钥")))}
            value={token}
            spellCheck={false}
            autoComplete="off"
            autoCapitalize="off"
            autoCorrect="off"
            disabled={pending}
            aria-invalid={Boolean(validation?.includes("密钥"))}
            onChange={(event) => {
              setToken(event.target.value);
              if (validation) {
                setValidation(null);
              }
            }}
          />
          <Button
            type="button"
            variant="secondary"
            className="shrink-0"
            disabled={pending}
            onClick={generateToken}
          >
            <RefreshCw size={14} aria-hidden="true" />
            生成
          </Button>
        </div>
      </Field>

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

      <div className="flex items-center justify-end gap-2 border-t border-subtle pt-4">
        <Button type="button" variant="secondary" className="min-w-[4.5rem]" disabled={pending} onClick={onClose}>
          取消
        </Button>
        <Button type="submit" variant="primary" disabled={pending}>
          保存
        </Button>
      </div>
    </form>
  );
}
