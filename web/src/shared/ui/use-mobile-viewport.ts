import { useSyncExternalStore } from "react";

const MOBILE_VIEWPORT_QUERY = "(max-width: 767px)";

function subscribe(listener: () => void) {
  const media = window.matchMedia?.(MOBILE_VIEWPORT_QUERY);
  media?.addEventListener("change", listener);
  return () => media?.removeEventListener("change", listener);
}

function isMobile() {
  return typeof window !== "undefined"
    && window.matchMedia?.(MOBILE_VIEWPORT_QUERY).matches === true;
}

export function useMobileViewport() {
  return useSyncExternalStore(subscribe, isMobile, () => false);
}
