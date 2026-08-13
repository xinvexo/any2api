import { beforeEach, describe, expect, it } from "vitest";

import {
  applyTheme,
  persistThemeMode,
  readThemeMode,
} from "./theme";

describe("theme state", () => {
  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
    delete document.documentElement.dataset.themeMode;
  });

  it("uses the default for an old value without rewriting browser state", () => {
    localStorage.setItem("any2api-theme", "system");

    const mode = readThemeMode();
    applyTheme(mode);

    expect(mode).toBe("light");
    expect(localStorage.getItem("any2api-theme")).toBe("system");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(document.documentElement.dataset.themeMode).toBeUndefined();
  });

  it("persists only an explicit current selection", () => {
    applyTheme("dark");
    expect(localStorage.getItem("any2api-theme")).toBeNull();

    persistThemeMode("dark");
    expect(localStorage.getItem("any2api-theme")).toBe("dark");
  });
});
