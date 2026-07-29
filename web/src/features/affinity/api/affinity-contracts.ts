export interface AffinityRuntime {
  configRevision: number;
  affinityEnabled: boolean;
  activeSessionCount: number;
  creatingSessionCount: number;
}

export function parseAffinityRuntime(value: unknown): AffinityRuntime {
  const record = readRecord(value);
  return {
    configRevision: readPositiveInteger(record.config_revision),
    affinityEnabled: readBoolean(record.affinity_enabled),
    activeSessionCount: readNonNegativeInteger(record.active_session_count),
    creatingSessionCount: readNonNegativeInteger(record.creating_session_count),
  };
}

function readRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null) {
    throw new Error("invalid affinity response");
  }
  return value as Record<string, unknown>;
}

function readPositiveInteger(value: unknown): number {
  const number = readNonNegativeInteger(value);
  if (number === 0) {
    throw new Error("invalid affinity response");
  }
  return number;
}

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new Error("invalid affinity response");
  }
  return value;
}

function readNonNegativeInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) < 0) {
    throw new Error("invalid affinity response");
  }
  return Number(value);
}
