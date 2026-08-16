import { useEffect, useEffectEvent } from "react";

export type LogChangeEventName =
  | "request_logs_changed"
  | "active_requests_changed"
  | "system_logs_changed";

const LOG_EVENTS_URL = "/api/admin/log-events";

export function useLogChangeEvent(
  eventName: LogChangeEventName | readonly LogChangeEventName[],
  enabled: boolean,
  onChange: () => void,
) {
  const handleChange = useEffectEvent(onChange);

  useEffect(() => {
    if (!enabled || typeof EventSource === "undefined") {
      return;
    }

    const source = new EventSource(LOG_EVENTS_URL);
    const eventNames = typeof eventName === "string" ? [eventName] : eventName;
    source.addEventListener("open", handleChange);
    eventNames.forEach((name) => source.addEventListener(name, handleChange));

    return () => {
      source.removeEventListener("open", handleChange);
      eventNames.forEach((name) => source.removeEventListener(name, handleChange));
      source.close();
    };
  }, [enabled, eventName]);
}
