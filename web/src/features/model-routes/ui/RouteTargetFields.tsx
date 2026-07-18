import { Trash2 } from "lucide-react";

import type { ProtocolDialect, ProviderEndpoint } from "@/features/providers";
import { Button } from "@/shared/ui/Button";

import type {
  RouteTargetEditorDraft,
  RouteTargetEditorErrors,
} from "../model/model-route-editor-types";

interface RouteTargetFieldsProps {
  index: number;
  target: RouteTargetEditorDraft;
  errors?: RouteTargetEditorErrors;
  endpoints: ProviderEndpoint[];
  ingressProtocol: ProtocolDialect;
  pending: boolean;
  canRemove: boolean;
  onUpdate: <Field extends "providerEndpointId" | "upstreamModel" | "fallbackTier" | "enabled">(
    field: Field,
    value: RouteTargetEditorDraft[Field],
  ) => void;
  onRemove: () => void;
}

export function RouteTargetFields({
  index,
  target,
  errors,
  endpoints,
  ingressProtocol,
  pending,
  canRemove,
  onUpdate,
  onRemove,
}: RouteTargetFieldsProps) {
  const compatibleEndpoints = endpoints.filter(
    (endpoint) => endpoint.protocolDialect === ingressProtocol,
  );
  const position = index + 1;
  const selectedEndpointAvailable = compatibleEndpoints.some(
    (endpoint) => endpoint.id === target.providerEndpointId,
  );
  const errorIds = {
    endpoint: `route-target-${target.clientId}-endpoint-error`,
    upstreamModel: `route-target-${target.clientId}-model-error`,
    fallbackTier: `route-target-${target.clientId}-tier-error`,
  };

  return (
    <fieldset className="space-y-4 border-t border-subtle py-5 first:border-t-0 first:pt-0">
      <legend className="sr-only">Target {position}</legend>
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="text-sm font-semibold">Target {position}</p>
          <p className="mt-1 text-xs text-tertiary">
            {target.fallbackTier === "0" ? "主 tier" : `Fallback tier ${target.fallbackTier || "-"}`}
          </p>
        </div>
        <Button
          variant="ghost"
          className="size-9 px-0"
          disabled={pending || !canRemove}
          aria-label={`删除 Target ${position}`}
          title={canRemove ? `删除 Target ${position}` : "至少保留一个 Target"}
          onClick={onRemove}
        >
          <Trash2 size={16} />
        </Button>
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        <Field
          label="Provider Endpoint"
          error={errors?.providerEndpointId}
          errorId={errorIds.endpoint}
        >
          <select
            className={inputClass}
            value={target.providerEndpointId}
            disabled={pending}
            aria-label={`Provider Endpoint ${position}`}
            aria-invalid={Boolean(errors?.providerEndpointId)}
            aria-describedby={errors?.providerEndpointId ? errorIds.endpoint : undefined}
            onChange={(event) => onUpdate("providerEndpointId", event.target.value)}
          >
            {!selectedEndpointAvailable ? (
              <option value={target.providerEndpointId}>
                {target.providerEndpointId ? "Endpoint 不可用" : "请选择 Endpoint"}
              </option>
            ) : null}
            {compatibleEndpoints.map((endpoint) => (
              <option key={endpoint.id} value={endpoint.id}>
                {endpoint.name}{endpoint.enabled ? "" : "（已停用）"}
              </option>
            ))}
          </select>
        </Field>

        <Field
          label="上游模型"
          error={errors?.upstreamModel}
          errorId={errorIds.upstreamModel}
        >
          <input
            className={inputClass}
            value={target.upstreamModel}
            maxLength={255}
            required
            disabled={pending}
            autoComplete="off"
            spellCheck={false}
            aria-label={`上游模型 ${position}`}
            aria-invalid={Boolean(errors?.upstreamModel)}
            aria-describedby={errors?.upstreamModel ? errorIds.upstreamModel : undefined}
            onChange={(event) => onUpdate("upstreamModel", event.target.value)}
          />
        </Field>
      </div>

      <div className="grid gap-4 sm:grid-cols-[minmax(0,180px)_minmax(0,1fr)] sm:items-end">
        <Field
          label="Fallback tier"
          error={errors?.fallbackTier}
          errorId={errorIds.fallbackTier}
        >
          <input
            className={inputClass}
            type="number"
            min={0}
            max={65_535}
            step={1}
            value={target.fallbackTier}
            required
            disabled={pending}
            inputMode="numeric"
            aria-label={`Fallback tier ${position}`}
            aria-invalid={Boolean(errors?.fallbackTier)}
            aria-describedby={errors?.fallbackTier ? errorIds.fallbackTier : undefined}
            onChange={(event) => onUpdate("fallbackTier", event.target.value)}
          />
        </Field>

        <label className="flex min-h-10 items-center gap-3 rounded-control border border-subtle bg-surface-muted px-4 py-2.5 text-sm font-medium">
          <input
            type="checkbox"
            className="size-4 accent-accent"
            checked={target.enabled}
            disabled={pending}
            aria-label={`启用 Target ${position}`}
            onChange={(event) => onUpdate("enabled", event.target.checked)}
          />
          启用此 Target
        </label>
      </div>
    </fieldset>
  );
}

function Field({
  label,
  error,
  errorId,
  children,
}: {
  label: string;
  error?: string;
  errorId: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <span className="text-sm font-medium">{label}</span>
      <div className="mt-2">{children}</div>
      {error ? <p id={errorId} className="mt-1.5 text-xs text-danger">{error}</p> : null}
    </div>
  );
}

const inputClass =
  "focus-ring h-10 w-full rounded-control border border-subtle bg-surface px-3 text-sm text-primary placeholder:text-tertiary disabled:opacity-60";
