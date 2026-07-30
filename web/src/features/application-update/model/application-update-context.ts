import { createContext, useContext } from "react";

export interface ApplicationUpdateContextValue {
  beginInstall: (targetVersion: string) => void;
  active: boolean;
}

export const ApplicationUpdateContext = createContext<ApplicationUpdateContextValue | null>(null);

export function useApplicationUpdateInstall() {
  const context = useContext(ApplicationUpdateContext);
  if (!context) {
    throw new Error("useApplicationUpdateInstall requires ApplicationUpdateProvider");
  }
  return context;
}
