import { useState } from "react";

import type {
  ProviderEndpoint,
  ProviderEndpointConfiguration,
  ProviderEndpointWriteInput,
  ProviderKind,
} from "../api/provider-contracts";
import { ProviderEndpointEditor } from "./ProviderEndpointEditor";
import { Button } from "@/shared/ui/Button";

export function ProviderEditorSlot({
  editorId,
  currentEndpoint,
  defaultKind,
  protocolOptions,
  configRevision,
  pending,
  error,
  onSubmit,
  onClose,
}: {
  editorId: string;
  currentEndpoint?: ProviderEndpoint;
  defaultKind: ProviderKind;
  protocolOptions: ProviderEndpointConfiguration["protocolOptions"];
  configRevision: number;
  pending: boolean;
  error: unknown;
  onSubmit: (input: ProviderEndpointWriteInput) => Promise<void>;
  onClose: () => void;
}) {
  const editing = editorId !== "new";
  const [initialEndpoint] = useState(currentEndpoint);

  if (editing && !initialEndpoint) {
    return (
      <div className="space-y-4 text-sm text-secondary">
        <p>Endpoint 不存在，该链接可能已经过期。</p>
        <Button onClick={onClose}>返回列表</Button>
      </div>
    );
  }

  const sourceConflict = editing
    ? !currentEndpoint
      ? "deleted"
      : currentEndpoint.configVersion !== initialEndpoint?.configVersion
        ? "changed"
        : null
    : null;

  return (
    <ProviderEndpointEditor
      endpoint={initialEndpoint}
      defaultKind={defaultKind}
      protocolOptions={protocolOptions}
      sourceConflict={sourceConflict}
      configRevision={configRevision}
      pending={pending}
      error={error}
      onSubmit={onSubmit}
      onClose={onClose}
    />
  );
}
