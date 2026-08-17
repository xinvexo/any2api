import { createContext, useContext, useEffect, useEffectEvent } from "react";

export type AdminEventName =
  | "overview_snapshot"
  | "request_logs_changed"
  | "active_requests_changed"
  | "system_logs_changed"
  | "oauth_quota_changed"
  | "oauth_refresh_diagnostic_changed"
  | (string & {});

export type AdminEventCallback = (payload: unknown) => void;

export interface AdminRealtimeStatus {
  connected: boolean;
  stale: boolean;
}

export interface AdminRealtimeContextValue {
  subscribe: (eventName: AdminEventName, callback: AdminEventCallback) => () => void;
  status: AdminRealtimeStatus;
}

export const AdminRealtimeContext = createContext<AdminRealtimeContextValue | null>(null);

export function useAdminEvent(
  eventName: AdminEventName,
  enabled: boolean,
  callback: AdminEventCallback,
) {
  const context = useAdminRealtimeContext();
  const { subscribe } = context;
  const handleEvent = useEffectEvent(callback);

  useEffect(() => {
    if (!enabled) {
      return;
    }
    return subscribe(eventName, (payload) => handleEvent(payload));
  }, [enabled, eventName, subscribe]);
}

export function useAdminRealtimeStatus() {
  return useAdminRealtimeContext().status;
}

function useAdminRealtimeContext() {
  const context = useContext(AdminRealtimeContext);
  if (!context) {
    throw new Error("admin realtime hooks must be used within AdminRealtimeProvider");
  }
  return context;
}
