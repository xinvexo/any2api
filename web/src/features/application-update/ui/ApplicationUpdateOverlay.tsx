import {
  ArrowDownToLine,
  Check,
  LockKeyhole,
  PackageCheck,
  RotateCw,
  TriangleAlert,
} from "lucide-react";
import { useEffect, useId, useRef, type CSSProperties } from "react";
import { createPortal } from "react-dom";

import type { ApplicationUpdateFlow } from "../model/update-flow";
import { Button } from "@/shared/ui/Button";

import "./application-update-overlay.css";

interface ApplicationUpdateOverlayProps {
  flow: Exclude<ApplicationUpdateFlow, { kind: "idle" }>;
  onRetry: () => void;
  onDismiss: () => void;
}

export function ApplicationUpdateOverlay({
  flow,
  onRetry,
  onDismiss,
}: ApplicationUpdateOverlayProps) {
  const titleId = useId();
  const descriptionId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);
  const view = getUpdateView(flow);

  useEffect(() => {
    dialogRef.current?.focus({ preventScroll: true });
  }, [flow.kind]);

  return createPortal(
    <div
      ref={dialogRef}
      className={`application-update-overlay is-${view.phase}`}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
      aria-busy={flow.kind === "running"}
      tabIndex={-1}
    >
      <section className="application-update-panel">
        <UpdateSymbol phase={view.phase} progress={view.progress} />

        <p className="application-update-eyebrow">any2api update</p>
        <h1 id={titleId} className="application-update-title">{view.title}</h1>
        <p id={descriptionId} className="application-update-description">{view.description}</p>

        {view.phase === "downloading" && view.progress !== null ? (
          <DownloadProgress
            progress={view.progress}
            downloadedBytes={view.downloadedBytes}
            totalBytes={view.totalBytes}
          />
        ) : null}

        {view.phase !== "failed" ? <StageTrack phase={view.phase} /> : null}

        {flow.kind === "failed" || flow.kind === "unconfirmed" ? (
          <div className="application-update-actions">
            <Button variant="secondary" size="lg" onClick={onDismiss}>返回</Button>
            <Button variant="primary" size="lg" onClick={onRetry}>
              {flow.kind === "unconfirmed" ? "继续等待" : "重新尝试"}
            </Button>
          </div>
        ) : (
          <p className="application-update-lock-note">
            {view.phase === "complete" ? (
              <Check size={13} aria-hidden="true" />
            ) : (
              <LockKeyhole size={13} aria-hidden="true" />
            )}
            {view.phase === "complete" ? "即将刷新管理页面" : "更新完成前请保持此页面打开"}
          </p>
        )}
      </section>
    </div>,
    document.body,
  );
}

function UpdateSymbol({ phase, progress }: { phase: ViewPhase; progress: number | null }) {
  const style = progress === null
    ? undefined
    : ({ "--application-update-progress": `${progress * 3.6}deg` } as CSSProperties);
  return (
    <div className="application-update-symbol" style={style} aria-hidden="true">
      <span className="application-update-symbol__ring" />
      <span className="application-update-symbol__core">
        {phase === "failed" ? <TriangleAlert /> : null}
        {phase === "complete" ? <Check /> : null}
        {phase === "checking" || phase === "downloading" ? <ArrowDownToLine /> : null}
        {phase === "installing" ? <PackageCheck /> : null}
        {phase === "restarting" ? <RotateCw /> : null}
      </span>
    </div>
  );
}

function DownloadProgress({
  progress,
  downloadedBytes,
  totalBytes,
}: {
  progress: number;
  downloadedBytes: number;
  totalBytes: number;
}) {
  return (
    <div className="application-update-download">
      <div
        className="application-update-progress"
        role="progressbar"
        aria-label="更新下载进度"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={progress}
      >
        <span style={{ width: `${progress}%` }} />
      </div>
      <div className="application-update-progress-meta">
        <span>{formatBytes(downloadedBytes)} / {formatBytes(totalBytes)}</span>
        <strong>{progress}%</strong>
      </div>
    </div>
  );
}

function StageTrack({ phase }: { phase: Exclude<ViewPhase, "failed"> }) {
  const current = phase === "checking" || phase === "downloading" ? 0 : phase === "installing" ? 1 : 2;
  return (
    <ol className="application-update-stages" aria-label="更新阶段">
      {["下载", "安装", "重启"].map((label, index) => (
        <li
          key={label}
          className={index < current || phase === "complete"
            ? "is-complete"
            : index === current ? "is-current" : undefined}
          aria-current={index === current && phase !== "complete" ? "step" : undefined}
        >
          <span aria-hidden="true" />
          {label}
        </li>
      ))}
    </ol>
  );
}

type ViewPhase = "checking" | "downloading" | "installing" | "restarting" | "complete" | "failed";

interface UpdateView {
  phase: ViewPhase;
  title: string;
  description: string;
  progress: number | null;
  downloadedBytes: number;
  totalBytes: number;
}

function getUpdateView(flow: Exclude<ApplicationUpdateFlow, { kind: "idle" }>): UpdateView {
  if (flow.kind === "complete") {
    return view("complete", "更新完成", `v${flow.targetVersion} 已成功启动。`);
  }
  if (flow.kind === "failed") {
    return view("failed", "更新未完成", flow.message);
  }
  if (flow.kind === "unconfirmed") {
    return view("failed", "无法确认更新结果", flow.message);
  }
  const { status, targetVersion } = flow;
  if (status.phase === "downloading") {
    const progress = Math.min(100, Math.floor(status.downloadedBytes / status.totalBytes * 100));
    return {
      ...view("downloading", `正在下载 v${status.targetVersion}`, "正在获取经过校验的官方版本。"),
      progress,
      downloadedBytes: status.downloadedBytes,
      totalBytes: status.totalBytes,
    };
  }
  if (status.phase === "installing") {
    return view("installing", "正在安装", `正在校验并安装 v${status.targetVersion}。`);
  }
  if (status.phase === "restarting") {
    return view("restarting", "正在重新启动", `正在等待 v${status.targetVersion} 恢复服务。`);
  }
  return view("checking", "正在准备更新", `正在验证 v${targetVersion} 的官方发布信息。`);
}

function view(phase: ViewPhase, title: string, description: string): UpdateView {
  return { phase, title, description, progress: null, downloadedBytes: 0, totalBytes: 0 };
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${value >= 10 ? value.toFixed(1) : value.toFixed(2)} ${unit}`;
}
