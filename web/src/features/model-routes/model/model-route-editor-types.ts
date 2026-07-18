import type { ProtocolDialect } from "@/features/providers";

export type FallbackSaturationDraft = "inherit" | "wait" | "fallback";

export interface RouteTargetEditorDraft {
  clientId: string;
  id?: string;
  originalProviderEndpointId?: string;
  originalUpstreamModel?: string;
  providerEndpointId: string;
  upstreamModel: string;
  fallbackTier: string;
  enabled: boolean;
}

export interface ModelRouteEditorDraft {
  publicModel: string;
  ingressProtocol: ProtocolDialect;
  fallbackOnSaturation: FallbackSaturationDraft;
  enabled: boolean;
  targets: RouteTargetEditorDraft[];
}

export interface RouteTargetEditorErrors {
  providerEndpointId?: string;
  upstreamModel?: string;
  fallbackTier?: string;
}

export interface ModelRouteEditorErrors {
  publicModel?: string;
  targets?: string;
  targetByClientId: Record<string, RouteTargetEditorErrors>;
}
