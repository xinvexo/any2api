export type ProxyKind = "direct" | "http" | "socks5";
export type EditableProxyKind = Exclude<ProxyKind, "direct">;

export interface ProxyProfile {
  id: string;
  name: string;
  kind: ProxyKind;
  host: string | null;
  port: number | null;
  enabled: boolean;
  builtIn: boolean;
  configVersion: number;
}

export interface ProxyConfiguration {
  configRevision: number;
  globalProxyId: string;
  items: ProxyProfile[];
}

export interface ProxyWriteInput {
  expectedRevision: number;
  name: string;
  kind: EditableProxyKind;
  host: string;
  port: number;
  enabled: boolean;
}

export function parseProxyConfiguration(value: unknown): ProxyConfiguration {
  if (!isRecord(value)) {
    throw new Error("invalid proxy configuration response");
  }
  const revision = readPositiveInteger(value.config_revision);
  const globalProxyId = readString(value.global_proxy_id);
  if (!Array.isArray(value.items)) {
    throw new Error("invalid proxy configuration response");
  }
  const items = value.items.map(parseProxyProfile);
  if (!items.some((item) => item.id === globalProxyId)) {
    throw new Error("invalid proxy configuration response");
  }

  return {
    configRevision: revision,
    globalProxyId,
    items,
  };
}

function parseProxyProfile(value: unknown): ProxyProfile {
  if (!isRecord(value)) {
    throw new Error("invalid proxy profile response");
  }
  const kind = readKind(value.kind);
  const host = readNullableString(value.host);
  const port = readNullablePort(value.port);
  const builtIn = readBoolean(value.built_in);
  if (kind === "direct" ? host !== null || port !== null || !builtIn : host === null || port === null) {
    throw new Error("invalid proxy profile response");
  }

  return {
    id: readString(value.id),
    name: readString(value.name),
    kind,
    host,
    port,
    enabled: readBoolean(value.enabled),
    builtIn,
    configVersion: readPositiveInteger(value.config_version),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error("invalid proxy configuration response");
  }
  return value;
}

function readNullableString(value: unknown): string | null {
  if (value === null) {
    return null;
  }
  return readString(value);
}

function readPositiveInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) <= 0) {
    throw new Error("invalid proxy configuration response");
  }
  return Number(value);
}

function readNullablePort(value: unknown): number | null {
  if (value === null) {
    return null;
  }
  const port = readPositiveInteger(value);
  if (port > 65_535) {
    throw new Error("invalid proxy configuration response");
  }
  return port;
}

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new Error("invalid proxy configuration response");
  }
  return value;
}

function readKind(value: unknown): ProxyKind {
  if (value !== "direct" && value !== "http" && value !== "socks5") {
    throw new Error("invalid proxy profile response");
  }
  return value;
}
