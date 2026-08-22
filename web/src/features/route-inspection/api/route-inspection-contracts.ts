import {
  PROTOCOL_DIALECTS,
  PROTOCOL_OPERATIONS,
  PROVIDER_KINDS,
  type ProtocolDialect,
  type ProtocolOperation,
  type ProviderKind,
} from "@/shared/api/provider-protocol-vocabulary";

export type RouteInspectionStatus = "available" | "no_enabled_candidate";

export type RouteProtocolDialect = ProtocolDialect;
export type RouteProtocolOperation = ProtocolOperation;
export type RouteProviderKind = ProviderKind;

export interface RouteInspectionCandidateGroup {
  providerKind: RouteProviderKind;
  providerEndpointId: string | null;
  providerEndpointName: string | null;
  upstreamProtocolDialect: RouteProtocolDialect;
  enabledCandidateCount: number;
}

export interface RouteInspectionOperation {
  operation: RouteProtocolOperation;
  candidateGroups: RouteInspectionCandidateGroup[];
}

export interface RouteInspectionItem {
  publicModel: string;
  ingressProtocol: RouteProtocolDialect;
  published: boolean;
  status: RouteInspectionStatus;
  operations: RouteInspectionOperation[];
}

export interface RouteInspection {
  configRevision: number;
  items: RouteInspectionItem[];
}

const STATUSES = ["available", "no_enabled_candidate"] as const;

export function parseRouteInspection(value: unknown): RouteInspection {
  const root = record(value);
  return {
    configRevision: positiveInteger(root.config_revision),
    items: array(root.items).map(parseItem),
  };
}

function parseItem(value: unknown): RouteInspectionItem {
  const item = record(value);
  return {
    publicModel: nonEmptyString(item.public_model),
    ingressProtocol: oneOf(item.ingress_protocol, PROTOCOL_DIALECTS),
    published: boolean(item.published),
    status: oneOf(item.status, STATUSES),
    operations: array(item.operations).map(parseOperation),
  };
}

function parseOperation(value: unknown): RouteInspectionOperation {
  const operation = record(value);
  return {
    operation: oneOf(operation.operation, PROTOCOL_OPERATIONS),
    candidateGroups: array(operation.candidate_groups).map(parseCandidateGroup),
  };
}

function parseCandidateGroup(value: unknown): RouteInspectionCandidateGroup {
  const group = record(value);
  return {
    providerKind: oneOf(group.provider_kind, PROVIDER_KINDS),
    providerEndpointId: nullableString(group.provider_endpoint_id),
    providerEndpointName: nullableString(group.provider_endpoint_name),
    upstreamProtocolDialect: oneOf(group.upstream_protocol_dialect, PROTOCOL_DIALECTS),
    enabledCandidateCount: positiveInteger(group.enabled_candidate_count),
  };
}

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw invalid();
  return value as Record<string, unknown>;
}

function array(value: unknown): unknown[] {
  if (!Array.isArray(value)) throw invalid();
  return value;
}

function boolean(value: unknown): boolean {
  if (typeof value !== "boolean") throw invalid();
  return value;
}

function nonEmptyString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) throw invalid();
  return value;
}

function nullableString(value: unknown): string | null {
  return value === null ? null : nonEmptyString(value);
}

function positiveInteger(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) throw invalid();
  return value;
}

function oneOf<const T extends string>(value: unknown, values: readonly T[]): T {
  if (typeof value !== "string" || !values.includes(value as T)) throw invalid();
  return value as T;
}

function invalid() {
  return new Error("invalid route inspection response");
}
