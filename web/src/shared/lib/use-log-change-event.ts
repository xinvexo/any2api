import { useEffect, useEffectEvent } from "react";

export type LogChangeEventName = "request_logs_changed" | "system_logs_changed";

const LOG_EVENTS_URL = "/api/admin/log-events";

export function useLogChangeEvent(
  eventName: LogChangeEventName,
  enabled: boolean,
  onChange: () => void,
) {
  const handleChange = useEffectEvent(onChange);

  useEffect(() => {
    if (!enabled || typeof EventSource === "undefined") {
      return;
    }

    const source = new EventSource(LOG_EVENTS_URL);
    source.addEventListener(eventName, handleChange);

    return () => {
      source.removeEventListener(eventName, handleChange);
      source.close();
    };
  }, [enabled, eventName]);
}
