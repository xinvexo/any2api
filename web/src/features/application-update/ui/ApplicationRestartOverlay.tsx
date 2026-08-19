import { Check, LockKeyhole, RotateCw, TriangleAlert } from "lucide-react";
import { useEffect, useId, useRef } from "react";
import { createPortal } from "react-dom";

import { Button } from "@/shared/ui/Button";

import type { ApplicationRestartFlow } from "../model/application-restart-flow";
import "./application-update-overlay.css";

interface ApplicationRestartOverlayProps {
  flow: Exclude<ApplicationRestartFlow, { kind: "idle" }>;
  onContinue: () => void;
  onDismiss: () => void;
}

export function ApplicationRestartOverlay({
  flow,
  onContinue,
  onDismiss,
}: ApplicationRestartOverlayProps) {
  const titleId = useId();
  const descriptionId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);
  const isRunning = flow.kind === "running";
  const isComplete = flow.kind === "complete";

  useEffect(() => {
    dialogRef.current?.focus({ preventScroll: true });
  }, [flow.kind]);

  return createPortal(
    <div
      ref={dialogRef}
      className={`application-update-overlay is-${isComplete ? "complete" : isRunning ? "restarting" : "failed"}`}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
      aria-busy={isRunning}
      tabIndex={-1}
    >
      <section className="application-update-panel">
        <RestartSymbol state={isComplete ? "complete" : isRunning ? "running" : "failed"} />
        <p className="application-update-eyebrow">ANY2API SERVICE</p>
        <h1 id={titleId} className="application-update-title">
          {isComplete ? "重启完成" : flow.kind === "unconfirmed" ? "无法确认重启结果" : "正在重新启动"}
        </h1>
        <p id={descriptionId} className="application-update-description">
          {isComplete
            ? "服务已恢复，即将刷新管理页面。"
            : flow.kind === "unconfirmed"
              ? flow.message
              : "等待 ANY2API 恢复服务。"}
        </p>
        {flow.kind === "unconfirmed" ? (
          <div className="application-update-actions">
            <Button variant="secondary" size="lg" onClick={onDismiss}>返回</Button>
            <Button variant="primary" size="lg" onClick={onContinue}>继续等待</Button>
          </div>
        ) : (
          <p className="application-update-lock-note">
            {isComplete ? <Check size={13} aria-hidden="true" /> : <LockKeyhole size={13} aria-hidden="true" />}
            {isComplete ? "即将刷新管理页面" : "重启完成前请保持此页面打开"}
          </p>
        )}
      </section>
    </div>,
    document.body,
  );
}

function RestartSymbol({ state }: { state: "running" | "complete" | "failed" }) {
  return (
    <div className="application-update-symbol" aria-hidden="true">
      <span className="application-update-symbol__ring" />
      <span className="application-update-symbol__core">
        {state === "running" ? <RotateCw /> : null}
        {state === "complete" ? <Check /> : null}
        {state === "failed" ? <TriangleAlert /> : null}
      </span>
    </div>
  );
}
