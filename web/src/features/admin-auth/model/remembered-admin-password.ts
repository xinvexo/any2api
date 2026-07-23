const STORAGE_KEY = "any2api.admin.remember-password";

/** Load a previously saved administrator password, if any. */
export function loadRememberedAdminPassword(): string | null {
  try {
    const value = window.localStorage.getItem(STORAGE_KEY);
    if (value === null || value.length === 0) {
      return null;
    }
    return value;
  } catch {
    return null;
  }
}

/** Persist the administrator password for the next login on this browser. */
export function saveRememberedAdminPassword(password: string) {
  try {
    window.localStorage.setItem(STORAGE_KEY, password);
  } catch {
    // Private mode / quota failures are non-fatal; login still succeeds.
  }
}

/** Clear any remembered administrator password. */
export function clearRememberedAdminPassword() {
  try {
    window.localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Ignore storage access failures.
  }
}
