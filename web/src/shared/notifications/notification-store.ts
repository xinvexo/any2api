/**
 * Process-level notification store.
 * Survives React tree remounts and route/menu switches; only NotificationHost
 * needs to stay mounted at the app root to render items.
 */

export type NotificationTone = "success" | "warning" | "danger" | "info";

export interface NotificationItem {
  id: string;
  message: string;
  tone: NotificationTone;
  durationMs: number;
  createdAt: number;
}

export interface ShowNotificationInput {
  message: string;
  tone?: NotificationTone;
  durationMs?: number;
}

const DEFAULT_DURATION_MS = 3_600;
/** Soft cap so a runaway loop cannot cover the screen; newest stay, oldest drop. */
const MAX_VISIBLE = 8;

type Listener = (items: readonly NotificationItem[]) => void;

let sequence = 0;
let items: NotificationItem[] = [];
const listeners = new Set<Listener>();
const dismissTimers = new Map<string, number>();

/**
 * Push a notification onto the global stack.
 * Multiple notifications coexist; each keeps its own auto-dismiss timer.
 */
export function showNotification(input: ShowNotificationInput | string): string {
  const message = typeof input === "string" ? input : input.message.trim();
  if (!message) {
    return "";
  }

  const tone = typeof input === "string" ? "info" : (input.tone ?? "info");
  const durationMs =
    typeof input === "string"
      ? DEFAULT_DURATION_MS
      : Math.max(1_200, input.durationMs ?? DEFAULT_DURATION_MS);

  sequence += 1;
  const id = `notification-${sequence}`;
  const next: NotificationItem = {
    id,
    message,
    tone,
    durationMs,
    createdAt: Date.now(),
  };

  // Newest first so concurrent feedback stacks without replacing siblings.
  const previousIds = new Set(items.map((item) => item.id));
  items = [next, ...items].slice(0, MAX_VISIBLE);
  for (const previousId of previousIds) {
    if (!items.some((item) => item.id === previousId)) {
      clearDismissTimer(previousId);
    }
  }
  scheduleDismiss(id, durationMs);
  emit();
  return id;
}

export function dismissNotification(id: string): void {
  if (!items.some((item) => item.id === id)) {
    return;
  }
  clearDismissTimer(id);
  items = items.filter((item) => item.id !== id);
  emit();
}

/** Test / logout helper. Production UI must not clear on menu navigation. */
export function clearNotifications(): void {
  for (const id of [...dismissTimers.keys()]) {
    clearDismissTimer(id);
  }
  items = [];
  emit();
}

export function getNotifications(): readonly NotificationItem[] {
  return items;
}

export function subscribeNotifications(listener: Listener): () => void {
  listeners.add(listener);
  listener(items);
  return () => {
    listeners.delete(listener);
  };
}

function scheduleDismiss(id: string, durationMs: number): void {
  if (typeof window === "undefined") {
    return;
  }
  clearDismissTimer(id);
  const handle = window.setTimeout(() => {
    dismissTimers.delete(id);
    dismissNotification(id);
  }, durationMs);
  dismissTimers.set(id, handle);
}

function clearDismissTimer(id: string): void {
  const handle = dismissTimers.get(id);
  if (handle === undefined) {
    return;
  }
  window.clearTimeout(handle);
  dismissTimers.delete(id);
}

function emit(): void {
  for (const listener of listeners) {
    listener(items);
  }
}
