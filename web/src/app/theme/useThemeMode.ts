import { useEffect, useState } from "react";

import { applyTheme, readThemeMode, type ThemeMode } from "@/app/theme/theme";

export function useThemeMode() {
  const [mode, setMode] = useState<ThemeMode>(readThemeMode);

  useEffect(() => {
    applyTheme(mode);
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const updateSystemTheme = () => mode === "system" && applyTheme("system");
    media.addEventListener("change", updateSystemTheme);
    return () => media.removeEventListener("change", updateSystemTheme);
  }, [mode]);

  return [mode, setMode] as const;
}
