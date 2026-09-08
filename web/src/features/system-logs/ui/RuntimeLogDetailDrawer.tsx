import { Link } from "react-router-dom";

import { diagnosticLabels, diagnosticValue, eventReason, eventTitle, levelLabels, moduleLabels } from "../model/runtime-log-presentation";
import type { RuntimeLogEntry } from "@/shared/api/generated/RuntimeLogEntry";
import type { RuntimeLogLevel } from "@/shared/api/generated/RuntimeLogLevel";
import { cn } from "@/shared/lib/cn";
import { formatCompactDateTime } from "@/shared/lib/date-time";
import { buttonClassName } from "@/shared/ui/button-class-name";
import { SideDrawer } from "@/shared/ui/SideDrawer";

const levelTones: Record<RuntimeLogLevel, string> = {
  error: "bg-danger/10 text-danger", warn: "bg-warning/10 text-warning", info: "bg-accent/10 text-accent",
  debug: "bg-surface-muted text-secondary", trace: "bg-surface-muted text-secondary",
};

export function RuntimeLogLevelBadge({ level }: { level: RuntimeLogLevel }) {
  return <span className={cn("inline-flex shrink-0 items-center justify-center rounded-md px-2 py-1 text-[12px] font-medium", levelTones[level])}>{levelLabels[level]}</span>;
}

export function RuntimeLogDetailDrawer({ entry, onClose }: { entry: RuntimeLogEntry | null; onClose: () => void }) {
  return (
    <SideDrawer open={entry !== null} title="运行日志详情" description={entry ? eventTitle(entry) : undefined} onClose={onClose} wide>
      {entry ? <div className="min-w-0 space-y-5">
        <div className="flex flex-wrap items-center gap-3 text-[12px] text-secondary">
          <RuntimeLogLevelBadge level={entry.level} />
          <span>{moduleLabels[entry.module] ?? entry.module}</span>
          <time dateTime={entry.timestamp}>{formatCompactDateTime(Date.parse(entry.timestamp))}</time>
        </div>
        {eventReason(entry) ? <p className="break-words rounded-xl bg-surface-muted/70 p-4 text-[13px] leading-6 [overflow-wrap:anywhere]">{eventReason(entry)}</p> : null}
        <dl className="grid grid-cols-1 gap-x-6 gap-y-4 text-[12px] sm:grid-cols-2">
          {Object.entries(entry.fields).map(([key, value]) => value === undefined ? null : <div key={key} className={cn("min-w-0", key === "error" && "sm:col-span-2")}>
            <dt className="text-secondary">{diagnosticLabels[key] ?? key}</dt>
            <dd className="mt-1 whitespace-pre-wrap break-words leading-5 text-primary [overflow-wrap:anywhere]">{diagnosticValue(key, value)}</dd>
          </div>)}
        </dl>
        {entry.fields.oauth_account_id ? <Link className={buttonClassName({ variant: "secondary" })} to="/oauth">查看 OAuth 账号</Link> : null}
        <details className="rounded-xl border border-subtle p-3 text-[12px]">
          <summary className="focus-ring cursor-pointer text-secondary">技术详情</summary>
          <p className="mt-3 whitespace-pre-wrap break-words leading-5 [overflow-wrap:anywhere]">{entry.message}</p>
          <p className="mt-2 break-all font-mono text-secondary">{entry.target}</p>
        </details>
      </div> : null}
    </SideDrawer>
  );
}
