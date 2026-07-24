import { useState, type FormEvent } from "react";

import type { OAuthAccount } from "../api/oauth-contracts";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { Field } from "@/shared/ui/form-field";

export function OAuthAccountEditor({
  account,
  mode,
  pending,
  error,
  onSaveMetadata,
  onSaveModels,
  onClose,
}: {
  account: OAuthAccount;
  mode: "metadata" | "models";
  pending: boolean;
  error: unknown;
  onSaveMetadata: (value: {
    label: string;
    maxConcurrency: number;
    enabled: boolean;
  }) => Promise<void>;
  onSaveModels: (models: string[]) => Promise<void>;
  onClose: () => void;
}) {
  const [label, setLabel] = useState(account.label);
  const [maxConcurrency, setMaxConcurrency] = useState(String(account.maxConcurrency));
  const [enabled, setEnabled] = useState(account.enabled);
  const [models, setModels] = useState(account.models.join("\n"));

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      if (mode === "metadata") {
        await onSaveMetadata({
          label: label.trim(),
          maxConcurrency: Number(maxConcurrency),
          enabled,
        });
      } else {
        await onSaveModels(parseModels(models));
      }
      onClose();
    } catch {
      // Keep the local form mounted after validation and revision conflicts.
    }
  }

  return (
    <form className="space-y-5" onSubmit={(event) => void submit(event)}>
      {mode === "metadata" ? (
        <>
          <Field label="账号名称" htmlFor="oauth-account-label">
            <input
              id="oauth-account-label"
              className={controlClass(false)}
              value={label}
              maxLength={100}
              disabled={pending}
              onChange={(event) => setLabel(event.target.value)}
            />
          </Field>
          <Field label="最大并发" htmlFor="oauth-account-concurrency">
            <input
              id="oauth-account-concurrency"
              type="number"
              min={1}
              max={10_000}
              className={controlClass(false)}
              value={maxConcurrency}
              disabled={pending}
              onChange={(event) => setMaxConcurrency(event.target.value)}
            />
          </Field>
          <label className="flex items-center gap-3 text-sm">
            <input
              type="checkbox"
              checked={enabled}
              disabled={pending}
              onChange={(event) => setEnabled(event.target.checked)}
            />
            启用这个 OAuth 账号
          </label>
        </>
      ) : (
        <Field
          label="已选模型"
          htmlFor="oauth-account-models"
          hint="每行一个 Provider 模型名；保存时会拒绝账号权限目录之外的模型。"
        >
          <textarea
            id="oauth-account-models"
            rows={10}
            spellCheck={false}
            className={controlClass(false, "min-h-52 resize-y font-mono")}
            value={models}
            disabled={pending}
            onChange={(event) => setModels(event.target.value)}
          />
        </Field>
      )}
      {error ? (
        <p className="text-sm text-danger" role="alert">
          {getOAuthErrorMessage(error)}
        </p>
      ) : null}
      <div className="flex justify-end gap-2">
        <Button type="button" variant="ghost" disabled={pending} onClick={onClose}>
          取消
        </Button>
        <Button
          type="submit"
          variant="primary"
          disabled={
            pending ||
            (mode === "metadata" &&
              (label.trim().length === 0 ||
                !Number.isInteger(Number(maxConcurrency)) ||
                Number(maxConcurrency) < 1))
          }
        >
          保存
        </Button>
      </div>
    </form>
  );
}

function parseModels(value: string) {
  return [...new Set(value.split(/[\n,]/).map((model) => model.trim()).filter(Boolean))].sort();
}
