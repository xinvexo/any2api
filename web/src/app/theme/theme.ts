export const themeModes = ["light", "system", "dark"] as const;

export type ThemeMode = (typeof themeModes)[number];

const THEME_COLOR = {
  light: "#f0f4f9",
  dark: "#0f1115",
} as const;

export function readThemeMode(): ThemeMode {
  try {
    const value = localStorage.getItem("any2api-theme");
    return themeModes.includes(value as ThemeMode) ? (value as ThemeMode) : "system";
  } catch {
    return "system";
  }
}

export function applyTheme(mode: ThemeMode) {
  const systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const resolved = mode === "system" ? (systemDark ? "dark" : "light") : mode;

  document.documentElement.dataset.theme = resolved;
  document.documentElement.dataset.themeMode = mode;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", THEME_COLOR[resolved]);

  try {
    localStorage.setItem("any2api-theme", mode);
  } catch {
    // Theme selection still applies for the current page when storage is unavailable.
  }
}
