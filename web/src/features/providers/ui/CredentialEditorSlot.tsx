import { useState } from "react";

import type { ProviderCredential } from "../api/provider-credential-contracts";
import {
  ProviderCredentialEditor,
  type CredentialEditorSubmission,
} from "./ProviderCredentialEditor";
import type { ProxyConfiguration } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";

export function CredentialEditorSlot({
  mode,
  currentCredential,
  configRevision,
  proxies,
  pending,
  error,
  onSubmit,
  onClose,
}: {
  mode: "create" | "edit";
  currentCredential?: ProviderCredential;
  configRevision: number;
  proxies: ProxyConfiguration;
  pending: boolean;
  error: unknown;
  onSubmit: (submission: CredentialEditorSubmission) => Promise<void>;
  onClose: () => void;
}) {
  const [initialCredential] = useState(currentCredential);

  if (mode !== "create" && !initialCredential) {
    return (
      <div className="space-y-4 text-sm text-secondary">
        <p>API Key 不存在，该链接可能已经过期。</p>
        <Button onClick={onClose}>关闭</Button>
      </div>
    );
  }

  const sourceConflict =
    mode === "create"
      ? null
      : !currentCredential
        ? "deleted"
        : currentCredential.configVersion !== initialCredential?.configVersion ||
            currentCredential.secretVersion !== initialCredential?.secretVersion
          ? "changed"
          : null;

  return (
    <ProviderCredentialEditor
      mode={mode}
      credential={initialCredential}
      sourceConflict={sourceConflict}
      configRevision={configRevision}
      proxies={proxies}
      pending={pending}
      error={error}
      onSubmit={onSubmit}
      onClose={onClose}
    />
  );
}
