import { createContext, useContext } from "react";

export interface ApplicationRestartContextValue {
  beginRestart: () => Promise<void>;
  pending: boolean;
  active: boolean;
}

export const ApplicationRestartContext = createContext<ApplicationRestartContextValue | null>(null);

export function useApplicationRestart() {
  const context = useContext(ApplicationRestartContext);
  if (!context) {
    throw new Error("useApplicationRestart requires ApplicationRestartProvider");
  }
  return context;
}
