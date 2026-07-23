export type OAuthProvider = "codex" | "claude";

export interface OAuthStartResult {
  provider: OAuthProvider;
  sessionId: string;
  authorizationUrl: string;
  redirectUri: string;
  expiresInSeconds: number;
}

export function parseOAuthStartResult(value: unknown): OAuthStartResult {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const provider = value.provider;
  if (provider !== "codex" && provider !== "claude") {
    throw invalidResponse();
  }
  const expiresInSeconds = value.expires_in_seconds;
  if (
    typeof expiresInSeconds !== "number" ||
    !Number.isInteger(expiresInSeconds) ||
    expiresInSeconds <= 0
  ) {
    throw invalidResponse();
  }
  return {
    provider,
    sessionId: readString(value.session_id),
    authorizationUrl: readHttpUrl(value.authorization_url),
    redirectUri: readHttpUrl(value.redirect_uri),
    expiresInSeconds,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown) {
  if (typeof value !== "string" || value.length === 0) {
    throw invalidResponse();
  }
  return value;
}

function readHttpUrl(value: unknown) {
  const text = readString(value);
  let url: URL;
  try {
    url = new URL(text);
  } catch {
    throw invalidResponse();
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw invalidResponse();
  }
  return text;
}

function invalidResponse() {
  return new Error("invalid OAuth2 login response");
}
