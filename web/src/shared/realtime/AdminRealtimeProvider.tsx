import {
  useCallback,
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import {
  AdminRealtimeContext,
  type AdminEventCallback,
  type AdminEventName,
  type AdminRealtimeStatus,
} from "./use-admin-event";
import { ADMIN_API_PREFIX } from "@/shared/api/paths";

const ADMIN_EVENTS_URL = `${ADMIN_API_PREFIX}/events`;
const RETRY_DELAYS_MS = [1_000, 2_000, 5_000] as const;
const RETRY_AFTER_REFRESH_MS = 5_000;
const STABLE_CONNECTION_MS = 30_000;
const SNAPSHOT_STALE_AFTER_MS = 7_000;

interface AdminRealtimeProviderProps {
  authenticated: boolean;
  onAuthRefresh?: () => Promise<void>;
  children: ReactNode;
}

export function AdminRealtimeProvider({
  authenticated,
  onAuthRefresh,
  children,
}: AdminRealtimeProviderProps) {
  const subscribersRef = useRef(new Map<AdminEventName, Set<AdminEventCallback>>());
  const sourceRef = useRef<EventSource | null>(null);
  const sourceListenersRef = useRef(new Map<AdminEventName, EventListener>());
  const snapshotStaleRef = useRef(true);
  const snapshotStaleTimerRef = useRef<number | null>(null);
  const [status, setStatus] = useState<AdminRealtimeStatus>({
    connected: false,
    stale: true,
  });
  const canRefresh = onAuthRefresh !== undefined;

  const clearSnapshotStaleTimer = useCallback(() => {
    if (snapshotStaleTimerRef.current !== null) {
      window.clearTimeout(snapshotStaleTimerRef.current);
      snapshotStaleTimerRef.current = null;
    }
  }, []);

  const dispatch = useCallback((eventName: AdminEventName, event: Event) => {
    const payload = decodeEventPayload(event);
    if (eventName === "overview_snapshot") {
      const stale = readSnapshotStaleness(payload);
      if (stale !== null) {
        snapshotStaleRef.current = stale;
        clearSnapshotStaleTimer();
        if (!stale) {
          snapshotStaleTimerRef.current = window.setTimeout(() => {
            snapshotStaleTimerRef.current = null;
            snapshotStaleRef.current = true;
            setStatus((current) => current.stale ? current : { ...current, stale: true });
          }, SNAPSHOT_STALE_AFTER_MS);
        }
      }
    }
    setStatus((current) =>
      current.connected && current.stale === snapshotStaleRef.current
        ? current
        : { connected: true, stale: snapshotStaleRef.current });
    const listeners = subscribersRef.current.get(eventName);
    if (!listeners) {
      return;
    }
    for (const listener of [...listeners]) {
      try {
        listener(payload);
      } catch {
        // One malformed feature handler must not disconnect the shared stream.
      }
    }
  }, [clearSnapshotStaleTimer]);

  const ensureSourceListener = useCallback(
    (source: EventSource, eventName: AdminEventName) => {
      if (sourceListenersRef.current.has(eventName)) {
        return;
      }
      const listener: EventListener = (event) => dispatch(eventName, event);
      sourceListenersRef.current.set(eventName, listener);
      source.addEventListener(eventName, listener);
    },
    [dispatch],
  );

  const subscribe = useCallback(
    (eventName: AdminEventName, callback: AdminEventCallback) => {
      const listeners = subscribersRef.current.get(eventName) ?? new Set<AdminEventCallback>();
      listeners.add(callback);
      subscribersRef.current.set(eventName, listeners);
      const source = sourceRef.current;
      if (source) {
        ensureSourceListener(source, eventName);
      }
      return () => {
        const current = subscribersRef.current.get(eventName);
        if (!current) {
          return;
        }
        current.delete(callback);
        if (current.size > 0) {
          return;
        }
        subscribersRef.current.delete(eventName);
        const listener = sourceListenersRef.current.get(eventName);
        if (listener && sourceRef.current) {
          sourceRef.current.removeEventListener(eventName, listener);
        }
        sourceListenersRef.current.delete(eventName);
      };
    },
    [ensureSourceListener],
  );

  const refreshAuth = useEffectEvent(onAuthRefresh ?? noopRefresh);

  useEffect(() => {
    if (!authenticated || typeof EventSource === "undefined") {
      return;
    }

    let disposed = false;
    let source: EventSource | null = null;
    let retryTimer: number | null = null;
    let stableTimer: number | null = null;
    let failures = 0;
    let refreshedAfterFailure = false;

    const clearRetryTimer = () => {
      if (retryTimer !== null) {
        window.clearTimeout(retryTimer);
        retryTimer = null;
      }
    };

    const clearStableTimer = () => {
      if (stableTimer !== null) {
        window.clearTimeout(stableTimer);
        stableTimer = null;
      }
    };

    const detach = () => {
      if (!source) {
        return;
      }
      for (const [eventName, listener] of sourceListenersRef.current) {
        source.removeEventListener(eventName, listener);
      }
      source.removeEventListener("open", handleOpen);
      source.removeEventListener("error", handleError);
      sourceListenersRef.current.clear();
      source.close();
      if (sourceRef.current === source) {
        sourceRef.current = null;
      }
      source = null;
    };

    const handleOpen = () => {
      clearStableTimer();
      stableTimer = window.setTimeout(() => {
        stableTimer = null;
        failures = 0;
        refreshedAfterFailure = false;
      }, STABLE_CONNECTION_MS);
      setStatus((current) =>
        current.connected && current.stale === snapshotStaleRef.current
          ? current
          : { connected: true, stale: snapshotStaleRef.current });
    };

    function handleError() {
      if (disposed) {
        return;
      }
      clearRetryTimer();
      clearStableTimer();
      clearSnapshotStaleTimer();
      detach();
      snapshotStaleRef.current = true;
      setStatus((current) =>
        !current.connected && current.stale ? current : { connected: false, stale: true });
      if (failures < RETRY_DELAYS_MS.length) {
        const delay = RETRY_DELAYS_MS[failures];
        failures += 1;
        retryTimer = window.setTimeout(() => {
          retryTimer = null;
          connect();
        }, delay);
        return;
      }
      if (refreshedAfterFailure || !canRefresh) {
        return;
      }
      refreshedAfterFailure = true;
      void refreshAuth()
        .then(() => {
          if (disposed) {
            return;
          }
          failures = 0;
          retryTimer = window.setTimeout(() => {
            retryTimer = null;
            connect();
          }, RETRY_AFTER_REFRESH_MS);
        })
        .catch(() => {
          // The auth provider owns the session transition; leave this stream stopped.
        });
    }

    function connect() {
      if (disposed || source) {
        return;
      }
      try {
        source = new EventSource(ADMIN_EVENTS_URL);
      } catch {
        handleError();
        return;
      }
      sourceRef.current = source;
      source.addEventListener("open", handleOpen);
      source.addEventListener("error", handleError);
      for (const eventName of subscribersRef.current.keys()) {
        ensureSourceListener(source, eventName);
      }
    }

    connect();
    return () => {
      disposed = true;
      clearRetryTimer();
      clearStableTimer();
      clearSnapshotStaleTimer();
      detach();
      snapshotStaleRef.current = true;
      setStatus({ connected: false, stale: true });
    };
  }, [authenticated, canRefresh, clearSnapshotStaleTimer, ensureSourceListener]);

  const value = useMemo(() => ({ subscribe, status }), [status, subscribe]);
  return <AdminRealtimeContext.Provider value={value}>{children}</AdminRealtimeContext.Provider>;
}

function decodeEventPayload(event: Event): unknown {
  const data = event instanceof MessageEvent ? event.data : undefined;
  if (typeof data !== "string") {
    return data;
  }
  try {
    return JSON.parse(data) as unknown;
  } catch {
    return data;
  }
}

function readSnapshotStaleness(payload: unknown): boolean | null {
  if (
    typeof payload !== "object"
    || payload === null
    || !("freshness" in payload)
  ) {
    return null;
  }
  if (payload.freshness === "fresh") return false;
  if (payload.freshness === "stale") return true;
  return null;
}

async function noopRefresh() {
  return undefined;
}
