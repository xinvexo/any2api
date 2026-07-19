export interface AdminSessionState {
  initialized: boolean;
  authenticated: boolean;
  csrfToken: string | null;
  remoteAccessEnabled: boolean;
  secureTransport: boolean;
  clientLoopback: boolean;
  throughTrustedProxy: boolean;
  plaintextHttpWarning: boolean;
}

export function parseAdminSessionState(value: unknown): AdminSessionState {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const initialized = readBoolean(value.initialized);
  const authenticated = readBoolean(value.authenticated);
  const csrfToken = value.csrf_token === null ? null : readString(value.csrf_token);
  const secureTransport = readBoolean(value.secure_transport);
  const clientLoopback = readBoolean(value.client_loopback);
  const plaintextHttpWarning = readBoolean(value.plaintext_http_warning);
  if (
    authenticated !== (csrfToken !== null) ||
    (authenticated && !initialized) ||
    (plaintextHttpWarning && (secureTransport || clientLoopback))
  ) {
    throw invalidResponse();
  }
  return {
    initialized,
    authenticated,
    csrfToken,
    remoteAccessEnabled: readBoolean(value.remote_access_enabled),
    secureTransport,
    clientLoopback,
    throughTrustedProxy: readBoolean(value.through_trusted_proxy),
    plaintextHttpWarning,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readBoolean(value: unknown) {
  if (typeof value !== "boolean") {
    throw invalidResponse();
  }
  return value;
}

function readString(value: unknown) {
  if (typeof value !== "string" || value.length === 0) {
    throw invalidResponse();
  }
  return value;
}

function invalidResponse() {
  return new Error("invalid administrator session response");
}
