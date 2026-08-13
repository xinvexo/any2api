export interface JsonRequestOptions {
  signal?: AbortSignal;
  timeoutMs?: number | null;
  method?: string;
  body?: unknown;
  headers?: Readonly<Record<string, string>>;
}

export const ADMIN_SESSION_EXPIRED_EVENT = "any2api:admin-session-expired";

export interface ApiErrorDiagnostic {
  tokenVersion: number;
  trigger: string;
  stage: string;
  reason: string;
  upstreamStatus: number | null;
  failureScope: string | null;
  occurredAt: number;
  reauthorizationRequired: boolean;
}

let adminCsrfToken: string | null = null;

export function setAdminCsrfToken(value: string | null) {
  adminCsrfToken = value;
}

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
    message: string,
    public readonly diagnostic: ApiErrorDiagnostic | null = null,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export async function requestJson<T>(
  path: string,
  {
    signal,
    timeoutMs = 10_000,
    method = "GET",
    body,
    headers: requestHeaders,
  }: JsonRequestOptions = {},
): Promise<T> {
  const controller = new AbortController();
  let timedOut = false;
  const timeout = timeoutMs === null
    ? null
    : window.setTimeout(() => {
        timedOut = true;
        controller.abort();
      }, timeoutMs);
  const forwardAbort = () => controller.abort(signal?.reason);
  if (signal?.aborted) {
    forwardAbort();
  } else {
    signal?.addEventListener("abort", forwardAbort, { once: true });
  }

  try {
    const headers: Record<string, string> = {
      ...requestHeaders,
      Accept: "application/json",
    };
    const formDataBody = isFormData(body);
    if (body !== undefined && !formDataBody) {
      headers["Content-Type"] = "application/json";
    }
    if (requiresAdminCsrf(path, method) && adminCsrfToken) {
      headers["X-CSRF-Token"] = adminCsrfToken;
    }
    const response = await fetch(path, {
      method,
      headers,
      body:
        body === undefined ? undefined : formDataBody ? body : JSON.stringify(body),
      credentials: "same-origin",
      signal: controller.signal,
    });

    if (response.status === 401 && isProtectedAdminRequest(path)) {
      expireAdminSession();
    }

    if (!response.ok) {
      const error = await readApiError(response, controller.signal);
      throw error;
    }
    if (response.status === 204) {
      return undefined as T;
    }

    return (await response.json()) as T;
  } catch (error) {
    if (timedOut && !signal?.aborted) {
      throw new Error("request timed out", { cause: error });
    }
    throw error;
  } finally {
    if (timeout !== null) {
      window.clearTimeout(timeout);
    }
    signal?.removeEventListener("abort", forwardAbort);
  }
}

function isFormData(value: unknown): value is FormData {
  return typeof FormData !== "undefined" && value instanceof FormData;
}

function expireAdminSession() {
  setAdminCsrfToken(null);
  window.dispatchEvent(new Event(ADMIN_SESSION_EXPIRED_EVENT));
}

function isProtectedAdminRequest(path: string) {
  return (
    path.startsWith("/api/admin/") &&
    ![
      "/api/admin/auth/session",
      "/api/admin/auth/setup",
      "/api/admin/auth/login",
    ].includes(path)
  );
}

function requiresAdminCsrf(path: string, method: string) {
  return (
    path.startsWith("/api/admin/") &&
    !["GET", "HEAD", "OPTIONS"].includes(method.toUpperCase())
  );
}

async function readApiError(response: Response, signal: AbortSignal): Promise<ApiError> {
  let value: unknown;
  try {
    value = await response.json();
  } catch (error) {
    if (signal.aborted || isAbortError(error)) {
      throw error;
    }
    return new ApiError(
      response.status,
      "http_error",
      `request failed with status ${response.status}`,
    );
  }

  if (
    typeof value === "object" &&
    value !== null &&
    "error" in value &&
    typeof value.error === "object" &&
    value.error !== null &&
    "code" in value.error &&
    typeof value.error.code === "string" &&
    "message" in value.error &&
    typeof value.error.message === "string"
  ) {
    return new ApiError(
      response.status,
      value.error.code,
      value.error.message,
      parseApiErrorDiagnostic(
        "diagnostic" in value.error ? value.error.diagnostic : undefined,
      ),
    );
  }

  return new ApiError(
    response.status,
    "http_error",
    `request failed with status ${response.status}`,
  );
}

function parseApiErrorDiagnostic(value: unknown): ApiErrorDiagnostic | null {
  if (value === undefined || value === null) {
    return null;
  }
  if (
    typeof value !== "object"
    || !("token_version" in value)
    || !("trigger" in value)
    || !("stage" in value)
    || !("reason" in value)
    || !("upstream_status" in value)
    || !("failure_scope" in value)
    || !("occurred_at" in value)
    || !("reauthorization_required" in value)
    || !isIntegerAtLeast(value.token_version, 1)
    || typeof value.trigger !== "string"
    || typeof value.stage !== "string"
    || typeof value.reason !== "string"
    || !isOptionalInteger(value.upstream_status, 100)
    || !isOptionalString(value.failure_scope)
    || !isIntegerAtLeast(value.occurred_at, 0)
    || typeof value.reauthorization_required !== "boolean"
  ) {
    return null;
  }
  return {
    tokenVersion: value.token_version,
    trigger: value.trigger,
    stage: value.stage,
    reason: value.reason,
    upstreamStatus: value.upstream_status,
    failureScope: value.failure_scope,
    occurredAt: value.occurred_at,
    reauthorizationRequired: value.reauthorization_required,
  };
}

function isIntegerAtLeast(value: unknown, minimum: number): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= minimum;
}

function isOptionalInteger(value: unknown, minimum: number): value is number | null {
  return value === null || isIntegerAtLeast(value, minimum);
}

function isOptionalString(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

function isAbortError(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "name" in error &&
    error.name === "AbortError"
  );
}
