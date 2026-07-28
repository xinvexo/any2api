export interface AffinityRuntime {
  configRevision: number;
  bindingCount: number;
  creatingCount: number;
}

export function parseAffinityRuntime(value: unknown): AffinityRuntime {
  const record = readRecord(value);
  return {
    configRevision: readPositiveInteger(record.config_revision),
    bindingCount: readNonNegativeInteger(record.binding_count),
    creatingCount: readNonNegativeInteger(record.creating_count),
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

function readNonNegativeInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) < 0) {
    throw new Error("invalid affinity response");
  }
  return Number(value);
}
