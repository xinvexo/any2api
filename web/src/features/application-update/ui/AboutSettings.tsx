import {
  CheckCircle2,
  Download,
  ExternalLink,
  LoaderCircle,
  RefreshCw,
} from "lucide-react";

import { getUpdateErrorMessage } from "../model/update-error";
import { useApplicationUpdate } from "../model/use-application-update";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function AboutSettings() {
  const update = useApplicationUpdate();

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
  const installed = update.install.data;

  function checkForUpdate() {
    update.install.reset();
    update.check.reset();
    update.check.mutate();
  }

  return (
    <div className="space-y-8" aria-busy={update.isPending}>
      <section aria-label="版本信息">
        <dl className="space-y-1">
          <div className="flex min-h-11 items-center justify-between gap-4 py-2">
            <dt className="text-sm text-secondary">当前版本</dt>
            <dd className="font-mono text-sm font-medium tabular-nums tracking-tight">
              v{about.currentVersion}
            </dd>
          </div>
          <div className="flex min-h-11 items-center justify-between gap-4 py-2">
            <dt className="text-sm text-secondary">源码仓库</dt>
            <dd>
              <a
                className="focus-ring inline-flex items-center gap-1.5 rounded-md text-sm font-medium text-accent transition-colors hover:text-accent-strong"
                href={about.repositoryUrl}
                target="_blank"
                rel="noreferrer"
              >
                xinvexo/any2api
                <ExternalLink size={13} aria-hidden="true" />
              </a>
            </dd>
          </div>
        </dl>
      </section>

      <section aria-labelledby="about-update-heading" className="space-y-3">
        <header className="flex flex-wrap items-center justify-between gap-3">
          <div className="min-w-0">
            <h2 id="about-update-heading" className="text-[15px] font-semibold tracking-tight">
              版本更新
            </h2>
            <p className="mt-1 text-xs leading-5 text-secondary">
              从 GitHub Release 检查并安装官方构建。
            </p>
          </div>
          <Button size="sm" onClick={checkForUpdate} disabled={update.isPending}>
            {update.check.isPending ? (
              <LoaderCircle size={14} className="animate-spin" />
            ) : (
              <RefreshCw size={14} />
            )}
            检查更新
          </Button>
        </header>

        <div className="rounded-[12px] bg-surface-muted px-4 py-4" aria-live="polite">
          {!checked && !update.check.error && !installed ? (
            <p className="text-sm text-secondary">尚未检查更新。</p>
          ) : null}
          {update.check.error ? (
            <p className="text-sm text-danger" role="alert">
              {getUpdateErrorMessage(update.check.error)}
            </p>
          ) : null}
          {checked ? (
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="min-w-0">
                <p className="text-sm font-medium tracking-tight">
                  {checked.updateAvailable
                    ? `发现新版本 v${checked.latestVersion}`
                    : "当前已是最新版本"}
                </p>
                <a
                  className="focus-ring mt-1.5 inline-flex items-center gap-1 rounded text-xs text-accent hover:text-accent-strong"
                  href={checked.releaseUrl}
                  target="_blank"
                  rel="noreferrer"
                >
                  查看 Release
                  <ExternalLink size={11} aria-hidden="true" />
                </a>
              </div>
              {checked.updateAvailable && !installed ? (
                <Button
                  variant="primary"
                  size="sm"
                  onClick={() => update.install.mutate()}
                  disabled={update.isPending}
                >
                  {update.install.isPending ? (
                    <LoaderCircle size={14} className="animate-spin" />
                  ) : (
                    <Download size={14} />
                  )}
                  更新到 v{checked.latestVersion}
                </Button>
              ) : null}
            </div>
          ) : null}
          {update.install.error ? (
            <p className="mt-3 text-sm text-danger" role="alert">
              {getUpdateErrorMessage(update.install.error)}
            </p>
          ) : null}
          {installed ? (
            <p className="mt-1 flex items-center gap-2 text-sm text-success" role="status">
              <CheckCircle2 size={15} aria-hidden="true" />
              v{installed.installedVersion} 已安装，服务正在优雅重启。
            </p>
          ) : null}
        </div>
      </section>
    </div>
  );
}
