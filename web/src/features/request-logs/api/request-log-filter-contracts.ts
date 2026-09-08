import type { RequestLogOutcome } from "./request-attempt-contracts";
import type { ProtocolOperation } from "@/shared/api/provider-protocol-vocabulary";
import type { RequestLogFilterOptionsResponse } from "@/shared/api/generated/RequestLogFilterOptionsResponse";
import type { StableFilterOption as StableRequestLogFilterOption } from "@/shared/api/generated/StableFilterOption";

export type { StableFilterOption as StableRequestLogFilterOption } from "@/shared/api/generated/StableFilterOption";

export type RequestLogOperation = ProtocolOperation;

export interface RequestLogFilters {
  outcome?: RequestLogOutcome;
  publicModel?: string;
  gatewayApiKeyId?: string;
}

export const EMPTY_REQUEST_LOG_FILTERS: RequestLogFilters = {};

export function hasActiveRequestLogFilters(filters: RequestLogFilters) {
  return Object.values(filters).some((value) => value !== undefined && value !== "");
}

export interface RequestLogFilterOptions {
  publicModels: string[];
  gatewayApiKeys: StableRequestLogFilterOption[];
}

export function parseRequestLogFilterOptions(value: unknown): RequestLogFilterOptions {
  const record = readRecord<RequestLogFilterOptionsResponse>(value);
  return {
    publicModels: readStringArray(record.public_models),
    gatewayApiKeys: readOptions(record.gateway_api_keys),
  };
}

function readOptions(value: unknown): StableRequestLogFilterOption[] {
  if (!Array.isArray(value)) {
    throw invalidResponse();
  }
  return value.map((option) => {
    const record = readRecord<StableRequestLogFilterOption>(option);
    if (typeof record.deleted !== "boolean") {
      throw invalidResponse();
    }
    return {
      id: readString(record.id),
      label: readString(record.label),
      deleted: record.deleted,
    };
  });
}

function readStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    throw invalidResponse();
  }
  return value.map(readString);
}

function readRecord<T extends object>(value: unknown): T {
  if (typeof value !== "object" || value === null) {
    throw invalidResponse();
  }
  return value as T;
}

function readString(value: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw invalidResponse();
  }
  return value;
}

function invalidResponse() {
  return new Error("invalid request log response");
}
