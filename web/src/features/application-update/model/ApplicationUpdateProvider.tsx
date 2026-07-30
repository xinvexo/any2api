import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type PropsWithChildren,
} from "react";

import { ApiError } from "@/shared/api/http-client";

import {
  getApplicationHealthVersion,
  getApplicationUpdateStatus,
  startApplicationUpdate,
} from "../api/update-api";
import type { UpdateStatus } from "../api/update-contracts";
import { ApplicationUpdateOverlay } from "../ui/ApplicationUpdateOverlay";
import { getUpdateErrorMessage, getUpdateFailureMessage } from "./update-error";
import type { ApplicationUpdateFlow } from "./update-flow";
import { reloadApplication } from "./reload-application";
import { ApplicationUpdateContext } from "./application-update-context";

const PENDING_TARGET_KEY = "any2api.application-update-target.v1";
const POLL_INTERVAL_MS = 450;
const COMPLETE_DELAY_MS = 800;

export function ApplicationUpdateProvider({ children }: PropsWithChildren) {
  const [flow, setFlow] = useState<ApplicationUpdateFlow>(initialFlow);
  const pollingTarget = flow.kind === "running" && flow.accepted
    ? flow.targetVersion
    : null;

  const beginInstall = useCallback((targetVersion: string) => {
    setPendingTarget(targetVersion);
    setFlow({
      kind: "running",
      targetVersion,
      accepted: false,
      status: { phase: "checking" },
    });
    void startApplicationUpdate().then(
      (status) => {
        setFlow((current) => current.kind === "running"
          ? { ...current, accepted: true, status }
          : current);
      },
      (error: unknown) => {
        if (isDefinitiveStartFailure(error)) {
          setPendingTarget(null);
          setFlow({ kind: "failed", targetVersion, message: getUpdateErrorMessage(error) });
          return;
        }
        setFlow((current) => current.kind === "running"
          ? { ...current, accepted: true }
          : current);
      },
    );
  }, []);

  const dismissFailure = useCallback(() => {
    setPendingTarget(null);
    setFlow((current) => current.kind === "failed" ? { kind: "idle" } : current);
  }, []);

  const retry = useCallback(() => {
    if (flow.kind === "failed") {
      beginInstall(flow.targetVersion);
    }
  }, [beginInstall, flow]);

  useUpdatePolling(pollingTarget, setFlow);
  useLockedPage(flow.kind !== "idle", flow.kind === "running");
  useCompletionReload(flow);

  const value = useMemo(() => ({
    beginInstall,
    active: flow.kind !== "idle" && flow.kind !== "failed",
  }), [beginInstall, flow.kind]);

  return (
    <ApplicationUpdateContext.Provider value={value}>
      {children}
      {flow.kind !== "idle" ? (
        <ApplicationUpdateOverlay flow={flow} onRetry={retry} onDismiss={dismissFailure} />
      ) : null}
    </ApplicationUpdateContext.Provider>
  );
}

function useUpdatePolling(
  targetVersion: string | null,
  setFlow: React.Dispatch<React.SetStateAction<ApplicationUpdateFlow>>,
) {
  useEffect(() => {
    if (!targetVersion) {
      return;
    }
    let cancelled = false;
    let timer = 0;
    let idleObservations = 0;

    const poll = async () => {
      const [statusResult, healthResult] = await Promise.allSettled([
        getApplicationUpdateStatus(),
        getApplicationHealthVersion(),
      ]);
      if (cancelled) {
        return;
      }
      if (healthResult.status === "fulfilled" && healthResult.value === targetVersion) {
        setPendingTarget(null);
        setFlow({ kind: "complete", targetVersion });
        return;
      }
      if (statusResult.status === "fulfilled") {
        const status = statusResult.value;
        idleObservations = status.phase === "idle" ? idleObservations + 1 : 0;
        if ((status.phase !== "idle" || idleObservations >= 3)
          && applyStatus(status, targetVersion, setFlow)) {
          return;
        }
      }
      timer = window.setTimeout(() => void poll(), POLL_INTERVAL_MS);
    };

    void poll();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [setFlow, targetVersion]);
}

function isDefinitiveStartFailure(error: unknown) {
  return error instanceof ApiError
    && error.code !== "update_in_progress"
    && error.status < 500;
}

function applyStatus(
  status: UpdateStatus,
  expectedVersion: string,
  setFlow: React.Dispatch<React.SetStateAction<ApplicationUpdateFlow>>,
) {
  if (status.phase === "failed") {
    setPendingTarget(null);
    setFlow({
      kind: "failed",
      targetVersion: status.targetVersion ?? expectedVersion,
      message: getUpdateFailureMessage(status.failureCode),
    });
    return true;
  }
  if (status.phase === "idle") {
    setPendingTarget(null);
    setFlow({
      kind: "failed",
      targetVersion: expectedVersion,
      message: "更新任务已中止，当前版本未发生变化。",
    });
    return true;
  }
  const targetVersion = "targetVersion" in status ? status.targetVersion : expectedVersion;
  if (targetVersion !== expectedVersion) {
    setPendingTarget(targetVersion);
  }
  setFlow({ kind: "running", targetVersion, accepted: true, status });
  return false;
}

function useLockedPage(overlayVisible: boolean, warnBeforeUnload: boolean) {
  useEffect(() => {
    if (!overlayVisible) {
      return;
    }
    const previousOverflow = document.body.style.overflow;
    const applicationRoot = document.getElementById("root");
    const rootWasInert = applicationRoot?.hasAttribute("inert") ?? false;
    document.body.style.overflow = "hidden";
    applicationRoot?.setAttribute("inert", "");
    return () => {
      document.body.style.overflow = previousOverflow;
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

function useCompletionReload(flow: ApplicationUpdateFlow) {
  useEffect(() => {
    if (flow.kind !== "complete") {
      return;
    }
    const timer = window.setTimeout(reloadApplication, COMPLETE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [flow.kind]);
}

function initialFlow(): ApplicationUpdateFlow {
  const targetVersion = getPendingTarget();
  return targetVersion
    ? { kind: "running", targetVersion, accepted: true, status: { phase: "checking" } }
    : { kind: "idle" };
}

function getPendingTarget() {
  try {
    return window.sessionStorage.getItem(PENDING_TARGET_KEY);
  } catch {
    return null;
  }
}

function setPendingTarget(targetVersion: string | null) {
  try {
    if (targetVersion) {
      window.sessionStorage.setItem(PENDING_TARGET_KEY, targetVersion);
    } else {
      window.sessionStorage.removeItem(PENDING_TARGET_KEY);
    }
  } catch {
    // The server-side task remains authoritative when browser storage is unavailable.
  }
}
