import { useCallback } from "react";
import { useSearchParams } from "react-router-dom";

import type { ProviderKind } from "../api/provider-contracts";
import { PROVIDER_KIND_OPTIONS } from "./provider-kind-catalog";
import { isProviderKind } from "@/shared/api/provider-protocol-vocabulary";

export interface ProviderRouteState {
  selectedKind: ProviderKind;
  endpointEditorId: string | null;
  credentialEndpointId: string | null;
  credentialId: string | null;
  credentialAction: "models" | null;
}

export type ProviderRouteEvent =
  | { type: "select-kind"; kind: ProviderKind }
  | { type: "open-endpoint-editor"; id: string; kind?: ProviderKind }
  | { type: "close-endpoint-editor"; expectedId?: string | null }
  | {
      type: "open-credential-editor";
      endpointId: string;
      credentialId: string;
      kind?: ProviderKind;
    }
  | { type: "open-credential-models"; endpointId: string; credentialId: string }
  | {
      type: "close-credential";
      endpointId: string;
      expectedCredentialId?: string | null;
    };

export function decodeProviderRouteState(searchParams: URLSearchParams): ProviderRouteState {
  const kind = searchParams.get("kind");
  return {
    selectedKind: isProviderKind(kind)
      ? kind
      : (PROVIDER_KIND_OPTIONS[0]?.kind ?? "codex"),
    endpointEditorId: nonEmptyParam(searchParams, "editor"),
    credentialEndpointId: nonEmptyParam(searchParams, "keys"),
    credentialId: nonEmptyParam(searchParams, "credential"),
    credentialAction: searchParams.get("action") === "models" ? "models" : null,
  };
}

export function transitionProviderRoute(
  current: URLSearchParams,
  event: ProviderRouteEvent,
): URLSearchParams {
  if (
    event.type === "close-endpoint-editor"
    && event.expectedId
    && current.get("editor") !== event.expectedId
  ) {
    return current;
  }
  if (
    event.type === "close-credential"
    && (
      current.get("keys") !== event.endpointId
      || (event.expectedCredentialId
        && current.get("credential") !== event.expectedCredentialId)
    )
  ) {
    return current;
  }

  const next = new URLSearchParams(current);
  switch (event.type) {
    case "select-kind":
      next.set("kind", event.kind);
      clearEndpointEditor(next);
      clearCredential(next);
      break;
    case "open-endpoint-editor":
      clearCredential(next);
      next.set("editor", event.id);
      if (event.kind) {
        next.set("kind", event.kind);
      }
      break;
    case "close-endpoint-editor":
      clearEndpointEditor(next);
      break;
    case "open-credential-editor":
      clearEndpointEditor(next);
      next.delete("action");
      next.set("keys", event.endpointId);
      next.set("credential", event.credentialId);
      if (event.kind) {
        next.set("kind", event.kind);
      }
      break;
    case "open-credential-models":
      clearEndpointEditor(next);
      next.set("keys", event.endpointId);
      next.set("credential", event.credentialId);
      next.set("action", "models");
      break;
    case "close-credential":
      clearCredential(next);
      break;
  }
  return next;
}

export function useProviderRouteState() {
  const [searchParams, setSearchParams] = useSearchParams();
  const state = decodeProviderRouteState(searchParams);
  const dispatch = useCallback(
    (event: ProviderRouteEvent) => {
      setSearchParams(
        (current) => transitionProviderRoute(current, event),
        { replace: true },
      );
    },
    [setSearchParams],
  );

  return {
    ...state,
    selectKind: (kind: ProviderKind) => dispatch({ type: "select-kind", kind }),
    openEndpointEditor: (id: string, kind?: ProviderKind) =>
      dispatch({ type: "open-endpoint-editor", id, kind }),
    closeEndpointEditor: (expectedId?: string | null) =>
      dispatch({ type: "close-endpoint-editor", expectedId }),
    openCredentialEditor: (
      endpointId: string,
      credentialId: string,
      kind?: ProviderKind,
    ) => dispatch({
      type: "open-credential-editor",
      endpointId,
      credentialId,
      kind,
    }),
    openCredentialModels: (endpointId: string, credentialId: string) =>
      dispatch({ type: "open-credential-models", endpointId, credentialId }),
    closeCredential: (endpointId: string, expectedCredentialId?: string | null) =>
      dispatch({ type: "close-credential", endpointId, expectedCredentialId }),
  };
}

function nonEmptyParam(searchParams: URLSearchParams, name: string) {
  const value = searchParams.get(name);
  return value && value.length > 0 ? value : null;
}

function clearEndpointEditor(searchParams: URLSearchParams) {
  searchParams.delete("editor");
}

function clearCredential(searchParams: URLSearchParams) {
  searchParams.delete("keys");
  searchParams.delete("credential");
  searchParams.delete("action");
}
