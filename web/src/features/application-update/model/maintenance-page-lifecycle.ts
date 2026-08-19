import { useEffect } from "react";

import { useBodyScrollLock } from "@/shared/ui/useBodyScrollLock";

import { reloadApplication } from "./reload-application";

const COMPLETE_DELAY_MS = 800;

export function useMaintenancePageLock(overlayVisible: boolean, warnBeforeUnload: boolean) {
  useBodyScrollLock(overlayVisible);

  useEffect(() => {
    if (!overlayVisible) {
      return;
    }
    const applicationRoot = document.getElementById("root");
    const rootWasInert = applicationRoot?.hasAttribute("inert") ?? false;
    applicationRoot?.setAttribute("inert", "");
    return () => {
      if (!rootWasInert) {
        applicationRoot?.removeAttribute("inert");
      }
    };
  }, [overlayVisible]);

  useEffect(() => {
    if (!warnBeforeUnload) {
      return;
    }
    const warn = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [warnBeforeUnload]);
}

export function useMaintenanceCompletionReload(completed: boolean) {
  useEffect(() => {
    if (!completed) {
      return;
    }
    const timer = window.setTimeout(reloadApplication, COMPLETE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [completed]);
}
