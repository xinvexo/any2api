export const APPLICATION_RESTART_PENDING_INSTANCE_KEY = "any2api.application-restart.v1";
export const APPLICATION_RESTART_CONFIRMATION_TIMEOUT_MS = 12 * 60 * 1_000;

export type ApplicationRestartFlow =
  | { kind: "idle" }
  | { kind: "running"; previousInstanceId: string }
  | { kind: "complete" }
  | { kind: "unconfirmed"; previousInstanceId: string; message: string };

export function initialApplicationRestartFlow(): ApplicationRestartFlow {
  const previousInstanceId = readPendingInstanceId();
  return previousInstanceId
    ? { kind: "running", previousInstanceId }
    : { kind: "idle" };
}

export function persistPendingRestart(previousInstanceId: string | null) {
  try {
    if (previousInstanceId) {
      window.sessionStorage.setItem(
        APPLICATION_RESTART_PENDING_INSTANCE_KEY,
        previousInstanceId,
      );
    } else {
      window.sessionStorage.removeItem(APPLICATION_RESTART_PENDING_INSTANCE_KEY);
    }
  } catch {
    // A missing tab marker must not change the already accepted server-side restart.
  }
}

function readPendingInstanceId() {
  try {
    const value = window.sessionStorage.getItem(APPLICATION_RESTART_PENDING_INSTANCE_KEY);
    if (value && UUID.test(value)) {
      return value;
    }
    if (value) {
      window.sessionStorage.removeItem(APPLICATION_RESTART_PENDING_INSTANCE_KEY);
    }
  } catch {
    // Browser storage may be unavailable; start with an unlocked page in that case.
  }
  return null;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
