import { useCallback, useEffect, useState } from "react";

import {
  applyTheme,
  persistThemeMode,
  readThemeMode,
  type ThemeMode,
} from "@/app/theme/theme";

export function useThemeMode() {
  const [mode, setMode] = useState<ThemeMode>(readThemeMode);

  useEffect(() => {
    applyTheme(mode);
  }, [mode]);

  const selectMode = useCallback((nextMode: ThemeMode) => {
    persistThemeMode(nextMode);
    setMode(nextMode);
  }, []);

  return [mode, selectMode] as const;
}
