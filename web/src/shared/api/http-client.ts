export interface JsonRequestOptions {
  signal?: AbortSignal;
  timeoutMs?: number;
  method?: string;
  body?: unknown;
}

export const ADMIN_SESSION_EXPIRED_EVENT = "any2api:admin-session-expired";

let adminCsrfToken: string | null = null;

export function setAdminCsrfToken(value: string | null) {
  adminCsrfToken = value;
}

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export async function requestJson<T>(
  path: string,
  { signal, timeoutMs = 10_000, method = "GET", body }: JsonRequestOptions = {},
): Promise<T> {
  const controller = new AbortController();
  let timedOut = false;
  const timeout = window.setTimeout(() => {
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
    const headers: Record<string, string> = { Accept: "application/json" };
    if (body !== undefined) {
      headers["Content-Type"] = "application/json";
    }
    if (requiresAdminCsrf(path, method) && adminCsrfToken) {
      headers["X-CSRF-Token"] = adminCsrfToken;
    }
    const response = await fetch(path, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
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
    window.clearTimeout(timeout);
    signal?.removeEventListener("abort", forwardAbort);
  }
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

export function getJson<T>(path: string, options: JsonRequestOptions = {}) {
  return requestJson<T>(path, options);
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
    return new ApiError(response.status, value.error.code, value.error.message);
  }

  return new ApiError(
    response.status,
    "http_error",
    `request failed with status ${response.status}`,
  );
}

function isAbortError(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "name" in error &&
    error.name === "AbortError"
  );
}
