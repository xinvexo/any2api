import { useState } from "react";

import type { ProtocolDialect, ProviderEndpoint } from "@/features/providers";

import type {
  ModelRoute,
  ModelRouteWriteInput,
  RouteTarget,
  RouteTargetWriteInput,
} from "../api/model-route-contracts";
import type {
  FallbackSaturationDraft,
  ModelRouteEditorDraft,
  ModelRouteEditorErrors,
  RouteTargetEditorDraft,
} from "./model-route-editor-types";
import {
  modelRouteEditorHasErrors,
  validateModelRouteDraft,
} from "./model-route-editor-validation";

type RouteField = Exclude<keyof ModelRouteEditorDraft, "targets" | "ingressProtocol">;
type TargetField = "providerEndpointId" | "upstreamModel" | "fallbackTier" | "enabled";

let nextClientId = 0;

export function useModelRouteEditor(
  endpoints: ProviderEndpoint[],
  route?: ModelRoute,
) {
  const [draft, setDraft] = useState<ModelRouteEditorDraft>(() => initialDraft(endpoints, route));
  const [expectedConfigVersion] = useState(route?.configVersion);
  const [publicModelEdited, setPublicModelEdited] = useState(Boolean(route));
  const [errors, setErrors] = useState<ModelRouteEditorErrors>({ targetByClientId: {} });
  const resolvedDraft = resolveDraftEndpoints(draft, endpoints);

  function update<Field extends RouteField>(field: Field, value: ModelRouteEditorDraft[Field]) {
    setDraft((current) => ({ ...current, [field]: value }));
    setErrors((current) => ({ ...current, [field]: undefined }));
    if (field === "publicModel") {
      setPublicModelEdited(true);
    }
  }

  function updateProtocol(ingressProtocol: ProtocolDialect) {
    const defaultEndpointId = firstEndpointId(endpoints, ingressProtocol);
    setDraft((current) => ({
      ...current,
      ingressProtocol,
      targets: current.targets.map((target) => ({
        ...target,
        providerEndpointId: endpointMatchesProtocol(
          endpoints,
          target.providerEndpointId,
          ingressProtocol,
        )
          ? target.providerEndpointId
          : defaultEndpointId,
      })),
    }));
    setErrors((current) => ({ ...current, targets: undefined, targetByClientId: {} }));
  }

  function updateTarget<Field extends TargetField>(
    clientId: string,
    field: Field,
    value: RouteTargetEditorDraft[Field],
  ) {
    setDraft((current) => {
      const targetIndex = current.targets.findIndex((target) => target.clientId === clientId);
      if (targetIndex < 0) {
        return current;
      }
      const targets = current.targets.map((target) =>
        target.clientId === clientId ? { ...target, [field]: value } : target,
      );
      const shouldFollowFirstTarget =
        field === "upstreamModel" && targetIndex === 0 && !publicModelEdited;
      return {
        ...current,
        publicModel: shouldFollowFirstTarget ? String(value) : current.publicModel,
        targets,
      };
    });
    setErrors((current) => ({
      ...current,
      publicModel:
        field === "upstreamModel" && !publicModelEdited ? undefined : current.publicModel,
      targets: undefined,
      targetByClientId: {
        ...current.targetByClientId,
        [clientId]: { ...current.targetByClientId[clientId], [field]: undefined },
      },
    }));
  }

  function addTarget() {
    setDraft((current) => ({
      ...current,
      targets: [
        ...current.targets,
        newTargetDraft(firstEndpointId(endpoints, current.ingressProtocol)),
      ],
    }));
    setErrors((current) => ({ ...current, targets: undefined }));
  }

  function removeTarget(clientId: string) {
    setDraft((current) => {
      if (current.targets.length <= 1) {
        return current;
      }
      const targets = current.targets.filter((target) => target.clientId !== clientId);
      return {
        ...current,
        publicModel:
          !publicModelEdited && current.targets[0]?.clientId === clientId
            ? (targets[0]?.upstreamModel ?? "")
            : current.publicModel,
        targets,
      };
    });
    setErrors((current) => {
      const targetByClientId = { ...current.targetByClientId };
      delete targetByClientId[clientId];
      return { ...current, targets: undefined, targetByClientId };
    });
  }

  function buildInput(expectedRevision: number): ModelRouteWriteInput | null {
    const currentDraft = resolveDraftEndpoints(draft, endpoints);
    const nextErrors = validateModelRouteDraft(currentDraft, endpoints);
    setErrors(nextErrors);
    if (modelRouteEditorHasErrors(nextErrors)) {
      return null;
    }
    return {
      expectedRevision,
      expectedConfigVersion,
      publicModel: currentDraft.publicModel,
      ingressProtocol: currentDraft.ingressProtocol,
      fallbackOnSaturation: parseFallbackPolicy(currentDraft.fallbackOnSaturation),
      enabled: currentDraft.enabled,
      targets: currentDraft.targets.map(buildTargetInput),
    };
  }

  return {
    draft: resolvedDraft,
    errors,
    update,
    updateProtocol,
    updateTarget,
    addTarget,
    removeTarget,
    buildInput,
  };
}

function initialDraft(
  endpoints: ProviderEndpoint[],
  route?: ModelRoute,
): ModelRouteEditorDraft {
  const ingressProtocol = route?.ingressProtocol ?? "openai_responses";
  return {
    publicModel: route?.publicModel ?? "",
    ingressProtocol,
    fallbackOnSaturation: fallbackDraft(route?.fallbackOnSaturation),
    enabled: route?.enabled ?? true,
    targets: route?.targets.map(existingTargetDraft) ?? [
      newTargetDraft(firstEndpointId(endpoints, ingressProtocol)),
    ],
  };
}

function existingTargetDraft(target: RouteTarget): RouteTargetEditorDraft {
  return {
    clientId: target.id,
    id: target.id,
    originalProviderEndpointId: target.providerEndpointId,
    originalUpstreamModel: target.upstreamModel,
    providerEndpointId: target.providerEndpointId,
    upstreamModel: target.upstreamModel,
    fallbackTier: String(target.fallbackTier),
    enabled: target.enabled,
  };
}

function newTargetDraft(providerEndpointId: string): RouteTargetEditorDraft {
  nextClientId += 1;
  return {
    clientId: `new-target-${nextClientId}`,
    providerEndpointId,
    upstreamModel: "",
    fallbackTier: "0",
    enabled: true,
  };
}

function buildTargetInput(target: RouteTargetEditorDraft): RouteTargetWriteInput {
  const identityUnchanged =
    target.id !== undefined &&
    target.providerEndpointId === target.originalProviderEndpointId &&
    target.upstreamModel === target.originalUpstreamModel;
  return {
    ...(identityUnchanged ? { id: target.id } : {}),
    providerEndpointId: target.providerEndpointId,
    upstreamModel: target.upstreamModel,
    fallbackTier: Number(target.fallbackTier),
    enabled: target.enabled,
  };
}

function firstEndpointId(endpoints: ProviderEndpoint[], protocol: ProtocolDialect) {
  return (
    endpoints.find((endpoint) => endpoint.protocolDialect === protocol && endpoint.enabled)?.id ??
    endpoints.find((endpoint) => endpoint.protocolDialect === protocol)?.id ??
    ""
  );
}

function endpointMatchesProtocol(
  endpoints: ProviderEndpoint[],
  endpointId: string,
  protocol: ProtocolDialect,
) {
  return endpoints.some(
    (endpoint) => endpoint.id === endpointId && endpoint.protocolDialect === protocol,
  );
}

function resolveDraftEndpoints(
  draft: ModelRouteEditorDraft,
  endpoints: ProviderEndpoint[],
): ModelRouteEditorDraft {
  const defaultEndpointId = firstEndpointId(endpoints, draft.ingressProtocol);
  if (defaultEndpointId.length === 0) {
    return draft;
  }
  let changed = false;
  const targets = draft.targets.map((target) => {
    if (
      target.id ||
      endpointMatchesProtocol(endpoints, target.providerEndpointId, draft.ingressProtocol)
    ) {
      return target;
    }
    changed = true;
    return { ...target, providerEndpointId: defaultEndpointId };
  });
  return changed ? { ...draft, targets } : draft;
}

function fallbackDraft(value: boolean | null | undefined): FallbackSaturationDraft {
  if (value === true) {
    return "fallback";
  }
  if (value === false) {
    return "wait";
  }
  return "inherit";
}

function parseFallbackPolicy(value: FallbackSaturationDraft): boolean | null {
  if (value === "fallback") {
    return true;
  }
  if (value === "wait") {
    return false;
  }
  return null;
}
