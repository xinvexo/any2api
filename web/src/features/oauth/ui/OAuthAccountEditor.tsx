import { useState, type FormEvent } from "react";

import type { OAuthAccount } from "../api/oauth-contracts";
import { presentOAuthAccount } from "../model/oauth-account-presentation";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { OAuthModelCatalog } from "./OAuthModelCatalog";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { Field } from "@/shared/ui/form-field";
import { Switch } from "@/shared/ui/Switch";

export function OAuthAccountEditor({
  account,
  mode,
  pending,
  error,
  onSaveMetadata,
  onClose,
}: {
  account: OAuthAccount;
  mode: "metadata" | "models";
  pending: boolean;
  error: unknown;
  onSaveMetadata: (value: {
    label: string;
    requestsPerMinute: number | null;
    enabled: boolean;
  }) => Promise<void>;
  onClose: () => void;
}) {
  const [label, setLabel] = useState(account.label);
  const [requestsPerMinute, setRequestsPerMinute] = useState(
    account.requestsPerMinute === null ? "" : String(account.requestsPerMinute),
  );
  const [enabled, setEnabled] = useState(account.enabled);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      await onSaveMetadata({
        label: label.trim(),
        requestsPerMinute: requestsPerMinute.length === 0 ? null : Number(requestsPerMinute),
        enabled,
      });
      onClose();
    } catch {
      // Keep the local form mounted after validation and revision conflicts.
    }
  }

  if (mode === "models") {
    return (
      <OAuthModelCatalog presentation={presentOAuthAccount(account)} onClose={onClose} />
    );
  }

  return (
    <form className="space-y-5" onSubmit={(event) => void submit(event)}>
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
      <Field label="RPM 限制" htmlFor="oauth-account-rpm">
        <input
          id="oauth-account-rpm"
          type="number"
          min={1}
          max={100_000}
          className={controlClass(false)}
          value={requestsPerMinute}
          placeholder="留空表示无限制"
          disabled={pending}
          onChange={(event) => setRequestsPerMinute(event.target.value)}
        />
      </Field>
      <div className="flex items-center justify-between gap-4">
        <p id="oauth-account-enabled-label" className="text-[13px] font-medium">
          启用此 OAuth 账号
        </p>
        <Switch
          id="oauth-account-enabled"
          checked={enabled}
          disabled={pending}
          aria-labelledby="oauth-account-enabled-label"
          onCheckedChange={setEnabled}
        />
      </div>
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
            label.trim().length === 0 ||
            (requestsPerMinute.length > 0 &&
              (!Number.isInteger(Number(requestsPerMinute)) ||
                Number(requestsPerMinute) < 1 ||
                Number(requestsPerMinute) > 100_000))
          }
        >
          保存
        </Button>
      </div>
    </form>
  );
}
