import { afterEach, beforeEach, expect, test, vi } from "vitest";

import {
  loadSystemLogAdminOperationsPreference,
  saveSystemLogAdminOperationsPreference,
  SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY,
} from "./system-log-admin-operations-preference";

beforeEach(() => window.localStorage.removeItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY));

afterEach(() => {
  vi.restoreAllMocks();
  window.localStorage.removeItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY);
});

test("loads the default and strictly recognizes a disabled preference", () => {
  expect(loadSystemLogAdminOperationsPreference()).toBe(true);

  window.localStorage.setItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY, "false");
  expect(loadSystemLogAdminOperationsPreference()).toBe(false);

  window.localStorage.setItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY, "invalid");
  expect(loadSystemLogAdminOperationsPreference()).toBe(true);
});

test("saves both choices", () => {
  saveSystemLogAdminOperationsPreference(false);
  expect(window.localStorage.getItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY)).toBe("false");

  saveSystemLogAdminOperationsPreference(true);
  expect(window.localStorage.getItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY)).toBe("true");
});

test("falls back safely when browser storage is unavailable", () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("storage unavailable");
  });
  expect(loadSystemLogAdminOperationsPreference()).toBe(true);

  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw new Error("storage unavailable");
  });
  expect(() => saveSystemLogAdminOperationsPreference(false)).not.toThrow();
});
