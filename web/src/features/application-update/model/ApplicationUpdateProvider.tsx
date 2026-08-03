import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type PropsWithChildren,
} from "react";

import { ApiError } from "@/shared/api/http-client";
import { useBodyScrollLock } from "@/shared/ui/useBodyScrollLock";

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

export const APPLICATION_UPDATE_PENDING_TARGET_KEY = "any2api.application-update-target.v1";
export const APPLICATION_UPDATE_CONFIRMATION_TIMEOUT_MS = 90_000;
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

  const dismissOutcome = useCallback(() => {
    setPendingTarget(null);
    setFlow((current) => current.kind === "failed" || current.kind === "unconfirmed"
      ? { kind: "idle" }
      : current);
  }, []);

  const retry = useCallback(() => {
    if (flow.kind === "failed") {
      beginInstall(flow.targetVersion);
      return;
    }
    if (flow.kind === "unconfirmed") {
      setPendingTarget(flow.targetVersion);
      setFlow({
        kind: "running",
        targetVersion: flow.targetVersion,
        accepted: true,
        status: { phase: "restarting", targetVersion: flow.targetVersion },
      });
    }
  }, [beginInstall, flow]);

  useUpdatePolling(pollingTarget, setFlow);
  useLockedPage(flow.kind !== "idle", flow.kind === "running");
  useCompletionReload(flow);

  const value = useMemo(() => ({
    beginInstall,
    active: flow.kind === "running" || flow.kind === "complete",
  }), [beginInstall, flow.kind]);

  return (
    <ApplicationUpdateContext.Provider value={value}>
      {children}
      {flow.kind !== "idle" ? (
        <ApplicationUpdateOverlay flow={flow} onRetry={retry} onDismiss={dismissOutcome} />
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
    let lastAuthoritativeObservation = Date.now();
    let observedVersion: string | null = null;

    const poll = async () => {
      const [statusResult, healthResult] = await Promise.allSettled([
        getApplicationUpdateStatus(),
        getApplicationHealthVersion(),
      ]);
      if (cancelled) {
        return;
      }
      if (healthResult.status === "fulfilled") {
        observedVersion = healthResult.value;
        if (observedVersion === targetVersion) {
          setPendingTarget(null);
          setFlow({ kind: "complete", targetVersion });
          return;
        }
      }
      if (statusResult.status === "fulfilled") {
        const status = statusResult.value;
        idleObservations = status.phase === "idle" ? idleObservations + 1 : 0;
        if ((status.phase !== "idle" || idleObservations >= 3)
          && applyStatus(status, targetVersion, setFlow)) {
          return;
        }
        if (status.phase !== "idle") {
          lastAuthoritativeObservation = Date.now();
        }
      } else {
        idleObservations = 0;
        if (Date.now() - lastAuthoritativeObservation
          >= APPLICATION_UPDATE_CONFIRMATION_TIMEOUT_MS) {
          setPendingTarget(null);
          setFlow({
            kind: "unconfirmed",
            targetVersion,
            message: unconfirmedMessage(targetVersion, observedVersion),
          });
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

function unconfirmedMessage(targetVersion: string, observedVersion: string | null) {
  const observed = observedVersion
    ? `当前可访问服务仍为 v${observedVersion}，`
    : "当前无法连接服务，";
  const timeoutSeconds = APPLICATION_UPDATE_CONFIRMATION_TIMEOUT_MS / 1_000;
  return `${observed}连续 ${timeoutSeconds} 秒未能确认 v${targetVersion} 的更新状态。更新可能仍在进行，你可以继续等待，或返回管理页面后稍后刷新。`;
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
    return window.sessionStorage.getItem(APPLICATION_UPDATE_PENDING_TARGET_KEY);
  } catch {
    return null;
  }
}

function setPendingTarget(targetVersion: string | null) {
  try {
    if (targetVersion) {
      window.sessionStorage.setItem(APPLICATION_UPDATE_PENDING_TARGET_KEY, targetVersion);
    } else {
      window.sessionStorage.removeItem(APPLICATION_UPDATE_PENDING_TARGET_KEY);
    }
  } catch {
    // The server-side task remains authoritative when browser storage is unavailable.
  }
}
