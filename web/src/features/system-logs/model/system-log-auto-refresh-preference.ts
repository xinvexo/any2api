export const SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY = "any2api.system-logs.auto-refresh.v1";

export function loadSystemLogAutoRefreshPreference(): boolean {
  try {
    const value = window.localStorage.getItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY);
    if (value === "false") {
      return false;
    }
    return true;
  } catch {
    return true;
  }
}

export function saveSystemLogAutoRefreshPreference(enabled: boolean) {
  try {
    window.localStorage.setItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY, String(enabled));
  } catch {
    // Storage can be unavailable in private browsing; the in-memory choice still applies.
  }
}
