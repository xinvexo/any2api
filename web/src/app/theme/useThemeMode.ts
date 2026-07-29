import { useEffect, useState } from "react";

import { applyTheme, readThemeMode, type ThemeMode } from "@/app/theme/theme";

export function useThemeMode() {
  const [mode, setMode] = useState<ThemeMode>(readThemeMode);

  useEffect(() => {
    applyTheme(mode);
  }, [mode]);

  return [mode, setMode] as const;
}
