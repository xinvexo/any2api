import { afterEach, beforeEach, expect, test, vi } from "vitest";

import {
  loadSystemLogAutoRefreshPreference,
  saveSystemLogAutoRefreshPreference,
  SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY,
} from "./system-log-auto-refresh-preference";

beforeEach(() => window.localStorage.removeItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY));

afterEach(() => {
  vi.restoreAllMocks();
  window.localStorage.removeItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY);
});

test("loads the default and strictly recognizes a disabled preference", () => {
  expect(loadSystemLogAutoRefreshPreference()).toBe(true);

  window.localStorage.setItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY, "false");
  expect(loadSystemLogAutoRefreshPreference()).toBe(false);

  window.localStorage.setItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY, "invalid");
  expect(loadSystemLogAutoRefreshPreference()).toBe(true);
});

test("saves both choices", () => {
  saveSystemLogAutoRefreshPreference(false);
  expect(window.localStorage.getItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY)).toBe("false");

  saveSystemLogAutoRefreshPreference(true);
  expect(window.localStorage.getItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY)).toBe("true");
});

test("falls back safely when browser storage is unavailable", () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("storage unavailable");
  });
  expect(loadSystemLogAutoRefreshPreference()).toBe(true);

  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw new Error("storage unavailable");
  });
  expect(() => saveSystemLogAutoRefreshPreference(false)).not.toThrow();
});
