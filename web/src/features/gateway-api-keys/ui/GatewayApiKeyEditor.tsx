import { useEffect, useRef, useState, type FormEvent } from "react";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { getGatewayApiKeyErrorMessage } from "../model/gateway-api-key-error";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { Field, FormError, FormNotice } from "@/shared/ui/form-field";
import { Switch } from "@/shared/ui/Switch";
import {
  draftSourceChanged,
  type VersionedDraft,
} from "@/shared/lib/versioned-draft";

export interface GatewayApiKeyEditorSubmit {
  expectedRevision: number;
  expectedConfigVersion: number | null;
  name: string;
  requestsPerMinute: number | null;
  enabled: boolean;
}

interface GatewayApiKeyEditorProps {
  apiKey?: GatewayApiKey;
  configRevision: number;
  pending: boolean;
  error: unknown;
  onSubmit: (input: GatewayApiKeyEditorSubmit) => Promise<void>;
  onClose: () => void;
}

export function GatewayApiKeyEditor({
  apiKey,
  configRevision,
  pending,
  error,
  onSubmit,
  onClose,
}: GatewayApiKeyEditorProps) {
  const [snapshot] = useState<VersionedDraft<GatewayApiKey | undefined>>(() => ({
    source: {
      configRevision,
      entityVersion: apiKey?.configVersion,
    },
    value: apiKey,
  }));
  const sourceApiKey = snapshot.value;
  const sourceConflict = draftSourceChanged(snapshot.source, {
    configRevision,
    entityVersion: apiKey?.configVersion,
  });
  const saveDisabled = pending || sourceConflict;
  const [name, setName] = useState(sourceApiKey?.name ?? "");
  const [requestsPerMinute, setRequestsPerMinute] = useState(
    sourceApiKey?.requestsPerMinute === null || sourceApiKey === undefined
      ? ""
      : String(sourceApiKey.requestsPerMinute),
  );
  const [enabled, setEnabled] = useState(sourceApiKey?.enabled ?? true);
  const [validation, setValidation] = useState<string | null>(null);
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    nameRef.current?.focus();
  }, []);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (sourceConflict) {
      return;
    }
    if (!name.trim() || name.trim() !== name) {
      setValidation("名称不能为空，且首尾不能包含空格。");
      nameRef.current?.focus();
      return;
    }
    const rpm = Number(requestsPerMinute);
    if (
      requestsPerMinute.length > 0 &&
      (!Number.isInteger(rpm) || rpm < 1 || rpm > 100_000)
    ) {
      setValidation("RPM 必须留空，或填写 1 到 100000 的整数。");
      return;
    }
    setValidation(null);
    try {
      await onSubmit({
        expectedRevision: snapshot.source.configRevision,
        expectedConfigVersion: snapshot.source.entityVersion ?? null,
        name,
        requestsPerMinute: requestsPerMinute.length === 0 ? null : rpm,
        enabled,
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
        label="RPM 限制"
        error={validation?.includes("RPM") ? validation : undefined}
        htmlFor="gateway-key-rpm"
      >
        <input
          id="gateway-key-rpm"
          className={controlClass(Boolean(validation?.includes("RPM")))}
          type="number"
          min={1}
          max={100_000}
          step={1}
          value={requestsPerMinute}
          placeholder="留空表示无限制"
          disabled={pending}
          aria-invalid={Boolean(validation?.includes("RPM"))}
          onChange={(event) => {
            setRequestsPerMinute(event.target.value);
            if (validation) {
              setValidation(null);
            }
          }}
        />
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

      {sourceConflict ? (
        <FormNotice tone="warning">
          服务器配置已更新。当前草稿基于旧版本，不能直接保存；请取消后重新编辑。
        </FormNotice>
      ) : null}
      <FormError>{error ? getGatewayApiKeyErrorMessage(error) : null}</FormError>

      <div className="flex items-center justify-end gap-2 border-t border-subtle pt-4">
        <Button type="button" variant="secondary" className="min-w-[4.5rem]" disabled={pending} onClick={onClose}>
          取消
        </Button>
        <Button type="submit" variant="primary" disabled={saveDisabled}>
          保存
        </Button>
      </div>
    </form>
  );
}
