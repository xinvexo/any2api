import { useMemo, type PropsWithChildren } from "react";

import { ApplicationRestartOverlay } from "../ui/ApplicationRestartOverlay";
import { ApplicationRestartContext } from "./application-restart-context";
import {
  useMaintenanceCompletionReload,
  useMaintenancePageLock,
} from "./maintenance-page-lifecycle";
import { useApplicationRestartFlow } from "./use-application-restart-flow";

interface ApplicationRestartProviderProps extends PropsWithChildren {
  updateActive: boolean;
}

export function ApplicationRestartProvider({
  children,
  updateActive,
}: ApplicationRestartProviderProps) {
  const restart = useApplicationRestartFlow(updateActive);
  useMaintenancePageLock(
    restart.flow.kind !== "idle",
    restart.flow.kind === "running",
  );
  useMaintenanceCompletionReload(restart.flow.kind === "complete");

  const value = useMemo(() => ({
    beginRestart: restart.beginRestart,
    pending: restart.pending,
    active: restart.active,
  }), [restart.active, restart.beginRestart, restart.pending]);

  return (
    <ApplicationRestartContext.Provider value={value}>
      {children}
      {restart.flow.kind !== "idle" ? (
        <ApplicationRestartOverlay
          flow={restart.flow}
          onContinue={restart.continueWaiting}
          onDismiss={restart.dismiss}
        />
      ) : null}
    </ApplicationRestartContext.Provider>
  );
}
