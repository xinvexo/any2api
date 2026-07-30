import {
  CheckCircle2,
  Download,
  ExternalLink,
  LoaderCircle,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import type { ReactNode } from "react";

import type { UpdateCheckResult } from "../api/update-contracts";
import { useApplicationUpdateInstall } from "../model/application-update-context";
import { getUpdateErrorMessage } from "../model/update-error";
import { useApplicationUpdate } from "../model/use-application-update";
import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function AboutSettings() {
  const update = useApplicationUpdate();
  const installation = useApplicationUpdateInstall();

  if (update.about.isPending && !update.about.data) {
    return (
      <div className="flex min-h-56 items-center justify-center text-sm text-secondary" aria-busy="true">
        正在读取版本信息
      </div>
    );
  }
  if (!update.about.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取版本信息</p>
        <p className="mt-2 text-sm text-secondary">{getUpdateErrorMessage(update.about.error)}</p>
        <Button className="mt-5" onClick={() => void update.about.refetch()}>
          <RefreshCw size={14} />
          重试
        </Button>
      </Surface>
    );
  }

  const about = update.about.data;
  const checked = update.check.data;

  function checkForUpdate() {
    update.check.reset();
    update.check.mutate();
  }

  return (
    <div className="space-y-6" aria-busy={update.isPending || installation.active}>
      <section aria-label="版本信息" className="space-y-1">
        <AboutRow
          label="当前版本"
          value={
            <span className="font-mono text-[12px] font-medium tabular-nums tracking-tight text-primary">
              v{about.currentVersion}
            </span>
          }
        />
        <AboutRow
          label="源码仓库"
          value={
            <a
              className="focus-ring inline-flex items-center gap-1 rounded-[6px] text-[12px] font-medium text-accent transition-colors hover:text-accent-strong"
              href={about.repositoryUrl}
              target="_blank"
              rel="noreferrer"
            >
              xinvexo/any2api
              <ExternalLink size={12} aria-hidden="true" />
            </a>
          }
        />
      </section>

      <section aria-labelledby="about-update-heading" className="space-y-1">
        <div className="grid gap-3 px-1 py-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-6">
          <div className="min-w-0">
            <h3 id="about-update-heading" className="text-[13px] font-medium text-primary">
              版本更新
            </h3>
            <p className="mt-0.5 text-[12px] leading-5 text-secondary">
              从 GitHub Release 检查并安装官方构建
            </p>
          </div>
          <div className="flex sm:justify-end">
            <Button size="sm" onClick={checkForUpdate} disabled={update.isPending || installation.active}>
              {update.check.isPending ? (
                <LoaderCircle size={14} className="animate-spin" />
              ) : (
                <RefreshCw size={14} />
              )}
              检查更新
            </Button>
          </div>
        </div>

        <div aria-live="polite">
          {update.check.error ? (
            <UpdateStatusCard tone="danger">
              <p className="text-[13px] font-medium text-danger" role="alert">
                {getUpdateErrorMessage(update.check.error)}
              </p>
            </UpdateStatusCard>
          ) : null}
          {checked ? (
            <UpdateResult
              checked={checked}
              busy={update.isPending || installation.active}
              onInstall={() => installation.beginInstall(checked.latestVersion)}
            />
          ) : null}
        </div>
      </section>
    </div>
  );
}

function AboutRow({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="grid gap-2 px-1 py-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-6">
      <div className="min-w-0">
        <h3 className="text-[13px] font-medium text-primary">{label}</h3>
      </div>
      <div className="flex min-w-0 sm:justify-end">{value}</div>
    </div>
  );
}

function UpdateResult({
  checked,
  busy,
  onInstall,
}: {
  checked: UpdateCheckResult;
  busy: boolean;
  onInstall: () => void;
}) {
  if (checked.updateAvailable) {
    return (
      <UpdateStatusCard tone="accent">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex min-w-0 items-start gap-2.5">
            <span className="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full bg-accent/12 text-accent">
              <Sparkles size={13} aria-hidden="true" />
            </span>
            <div className="min-w-0">
              <p className="text-[13px] font-medium tracking-tight text-primary">
                发现新版本 v{checked.latestVersion}
              </p>
              <a
                className="focus-ring mt-1 inline-flex items-center gap-1 rounded text-[12px] text-accent hover:text-accent-strong"
                href={checked.releaseUrl}
                target="_blank"
                rel="noreferrer"
              >
                查看 Release
                <ExternalLink size={11} aria-hidden="true" />
              </a>
            </div>
          </div>
          <Button variant="primary" size="sm" onClick={onInstall} disabled={busy}>
            <Download size={14} />
            更新到 v{checked.latestVersion}
          </Button>
        </div>
      </UpdateStatusCard>
    );
  }

  return (
    <UpdateStatusCard tone="success">
      <div className="flex items-start gap-2.5">
        <span className="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full bg-success/12 text-success">
          <CheckCircle2 size={13} aria-hidden="true" />
        </span>
        <div className="min-w-0">
          <p className="text-[13px] font-medium tracking-tight text-primary">当前已是最新版本</p>
          <a
            className="focus-ring mt-1 inline-flex items-center gap-1 rounded text-[12px] text-secondary transition-colors hover:text-accent"
            href={checked.releaseUrl}
            target="_blank"
            rel="noreferrer"
          >
            查看 Release
            <ExternalLink size={11} aria-hidden="true" />
          </a>
        </div>
      </div>
    </UpdateStatusCard>
  );
}

function UpdateStatusCard({
  tone,
  children,
}: {
  tone: "accent" | "success" | "danger";
  children: ReactNode;
}) {
  return (
    <div
      className={cn(
        "rounded-[10px] px-3.5 py-3",
        tone === "accent" && "bg-accent/8",
        tone === "success" && "bg-success/8",
        tone === "danger" && "bg-danger/8",
      )}
    >
      {children}
    </div>
  );
}
