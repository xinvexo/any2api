import { SystemLogManagement } from "../features/system-logs/ui/SystemLogManagement";
import { RuntimeLogManagement } from "../features/system-logs/ui/RuntimeLogManagement";
import { Link, useSearchParams } from "react-router-dom";
import { Activity, Globe } from "lucide-react";
import { cn } from "@/shared/lib/cn";
import { SlidingSelectionIndicator } from "@/shared/ui/SlidingSelectionIndicator";

export function SystemLogsPage() {
  const [params] = useSearchParams();
  const tab = params.get("tab") === "http" ? "http" : "runtime";
  return <div className="flex min-w-0 flex-1 flex-col gap-4 md:h-full md:min-h-0">
    <nav aria-label="日志类型" className="relative isolate flex w-fit shrink-0 gap-1 rounded-full bg-surface-muted/60 p-1">
      <SlidingSelectionIndicator selected={tab} className="rounded-full bg-nav-active" />
      {[{ key: "runtime", label: "运行日志", icon: Activity }, { key: "http", label: "HTTP 访问", icon: Globe }].map(({ key, label, icon: Icon }) => <Link key={key} to={key === "http" ? "?tab=http" : "?"} aria-current={tab === key ? "page" : undefined} data-sliding-selection-item={key} className={cn("focus-ring relative z-10 inline-flex min-h-9 items-center gap-2 rounded-full px-4 py-2 text-[13px] font-medium pointer-coarse:min-h-10", tab === key ? "text-nav-active-fg" : "text-secondary hover:text-primary")}><Icon size={15} />{label}</Link>)}
    </nav>
    {tab === "runtime" ? <RuntimeLogManagement /> : <SystemLogManagement />}
  </div>;
}
