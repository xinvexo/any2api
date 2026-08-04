export type ThemeMode = "light" | "dark";

const THEME_COLOR = {
  light: "#f0f4f9",
  dark: "#0f1115",
} as const;

export function readThemeMode(): ThemeMode {
  try {
    const value = localStorage.getItem("any2api-theme");
    if (value === "light" || value === "dark") {
      return value;
    }
  } catch {
    // fall through
  }
  return "light";
}

export function applyTheme(mode: ThemeMode) {
  document.documentElement.dataset.theme = mode;
  document.documentElement.dataset.themeMode = mode;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", THEME_COLOR[mode]);

  try {
    localStorage.setItem("any2api-theme", mode);
  } catch {
    // Theme selection still applies for the current page when storage is unavailable.
  }
}
