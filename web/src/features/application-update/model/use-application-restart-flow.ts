import { useCallback, useEffect, useRef, useState } from "react";

import { ApiError } from "@/shared/api/http-client";

import {
  InvalidApplicationRestartResponseError,
  startApplicationRestart,
} from "../api/restart-api";
import { getApplicationHealth } from "../api/update-api";
import {
  APPLICATION_RESTART_CONFIRMATION_TIMEOUT_MS,
  initialApplicationRestartFlow,
  persistPendingRestart,
  type ApplicationRestartFlow,
} from "./application-restart-flow";

const POLL_INTERVAL_MS = 450;

export function useApplicationRestartFlow(updateActive: boolean) {
  const [flow, setFlow] = useState<ApplicationRestartFlow>(initialApplicationRestartFlow);
  const [pending, setPending] = useState(false);
  const requestInFlight = useRef(false);

  const beginRestart = useCallback(async () => {
    if (requestInFlight.current || flow.kind !== "idle") {
      return;
    }
    if (updateActive) {
      throw new Error("版本更新正在进行，暂时无法重启。");
    }

    requestInFlight.current = true;
    setPending(true);
    try {
      const health = await getApplicationHealth();
      persistPendingRestart(health.instanceId);
      try {
        await startApplicationRestart();
      } catch (error) {
        if (error instanceof ApiError || error instanceof InvalidApplicationRestartResponseError) {
          persistPendingRestart(null);
          throw error;
        }
        // The request may have reached the old process before the connection was lost.
      }
      setFlow({ kind: "running", previousInstanceId: health.instanceId });
    } finally {
      requestInFlight.current = false;
      setPending(false);
    }
  }, [flow.kind, updateActive]);

  const continueWaiting = useCallback(() => {
    setFlow((current) => current.kind === "unconfirmed"
      ? { kind: "running", previousInstanceId: current.previousInstanceId }
      : current);
  }, []);

  const dismiss = useCallback(() => {
    persistPendingRestart(null);
    setFlow((current) => current.kind === "unconfirmed" ? { kind: "idle" } : current);
  }, []);

  useRestartPolling(flow, setFlow);

  return {
    flow,
    beginRestart,
    continueWaiting,
    dismiss,
    pending,
    active: flow.kind !== "idle",
  };
}

function useRestartPolling(
  flow: ApplicationRestartFlow,
  setFlow: React.Dispatch<React.SetStateAction<ApplicationRestartFlow>>,
) {
  const previousInstanceId = flow.kind === "running" ? flow.previousInstanceId : null;

  useEffect(() => {
    if (!previousInstanceId) {
      return;
    }
    let cancelled = false;
    let timer = 0;
    const startedAt = Date.now();

    const poll = async () => {
      try {
        const health = await getApplicationHealth();
        if (cancelled) {
          return;
        }
        if (health.instanceId !== previousInstanceId) {
          persistPendingRestart(null);
          setFlow({ kind: "complete" });
          return;
        }
      } catch {
        if (cancelled) {
          return;
        }
      }

      if (Date.now() - startedAt >= APPLICATION_RESTART_CONFIRMATION_TIMEOUT_MS) {
        const minutes = APPLICATION_RESTART_CONFIRMATION_TIMEOUT_MS / 60_000;
        setFlow({
          kind: "unconfirmed",
          previousInstanceId,
          message: `连续 ${minutes} 分钟未能确认新的服务实例。重启可能仍在进行，你可以继续等待，或返回管理页面后稍后刷新。`,
        });
        return;
      }
      timer = window.setTimeout(() => void poll(), POLL_INTERVAL_MS);
    };

    void poll();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [previousInstanceId, setFlow]);
}
