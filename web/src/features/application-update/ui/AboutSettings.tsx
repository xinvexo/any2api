import {
  CheckCircle2,
  Download,
  ExternalLink,
  GitFork,
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
      <section aria-labelledby="about-application-heading">
        <header className="mb-2">
          <h2 id="about-application-heading" className="text-[15px] font-semibold tracking-tight">
            any2api
          </h2>
        </header>
        <dl className="divide-y divide-subtle border-y border-subtle text-[13px]">
          <InfoRow label="当前版本" value={`v${about.currentVersion}`} />
          <div className="flex min-h-12 items-center justify-between gap-4 py-3">
            <dt className="text-secondary">GitHub</dt>
            <dd>
              <a
                className="focus-ring inline-flex items-center gap-1.5 rounded text-accent hover:underline"
                href={about.repositoryUrl}
                target="_blank"
                rel="noreferrer"
              >
                <GitFork size={14} aria-hidden="true" />
                xinvexo/any2api
                <ExternalLink size={12} aria-hidden="true" />
              </a>
            </dd>
          </div>
        </dl>
      </section>

      <section aria-labelledby="about-update-heading">
        <header className="mb-4 flex flex-wrap items-center justify-between gap-3">
          <h2 id="about-update-heading" className="text-[15px] font-semibold tracking-tight">
            版本更新
          </h2>
          <Button onClick={checkForUpdate} disabled={update.isPending}>
            {update.check.isPending ? (
              <LoaderCircle size={14} className="animate-spin" />
            ) : (
              <RefreshCw size={14} />
            )}
            检查更新
          </Button>
        </header>

        <div className="border-y border-subtle py-4" aria-live="polite">
          {!checked && !update.check.error && !installed ? (
            <p className="text-[13px] text-secondary">尚未检查更新。</p>
          ) : null}
          {update.check.error ? (
            <p className="text-[13px] text-danger" role="alert">
              {getUpdateErrorMessage(update.check.error)}
            </p>
          ) : null}
          {checked ? (
            <div className="space-y-4">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <p className="text-[13px] font-medium">
                    {checked.updateAvailable
                      ? `发现新版本 v${checked.latestVersion}`
                      : "当前已是最新版本"}
                  </p>
                  <a
                    className="focus-ring mt-1 inline-flex items-center gap-1 rounded text-[12px] text-accent hover:underline"
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
            </div>
          ) : null}
          {update.install.error ? (
            <p className="mt-3 text-[13px] text-danger" role="alert">
              {getUpdateErrorMessage(update.install.error)}
            </p>
          ) : null}
          {installed ? (
            <p className="flex items-center gap-2 text-[13px] text-success" role="status">
              <CheckCircle2 size={15} aria-hidden="true" />
              v{installed.installedVersion} 已安装，服务正在优雅重启。
            </p>
          ) : null}
        </div>
      </section>
    </div>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex min-h-12 items-center justify-between gap-4 py-3">
      <dt className="text-secondary">{label}</dt>
      <dd className="font-medium">{value}</dd>
    </div>
  );
}
