import { useSearchParams } from "react-router-dom";

import { RuntimeLogManagement } from "../features/system-logs/ui/RuntimeLogManagement";
import { SystemLogManagement } from "../features/system-logs/ui/SystemLogManagement";
import { PageTabs } from "@/shared/ui/PageTabs";

const tabs = [
  { label: "运行日志", path: "/system-logs" },
  { label: "HTTP 访问", path: "/system-logs?tab=http" },
];

export function SystemLogsPage() {
  const [params] = useSearchParams();
  const tab = params.get("tab") === "http" ? "http" : "runtime";
  return <div className="flex min-w-0 flex-1 flex-col gap-4 md:h-full md:min-h-0">
    <div className="shrink-0">
      <PageTabs items={tabs} ariaLabel="日志类型" activePath={tab === "http" ? tabs[1].path : tabs[0].path} />
    </div>
    {tab === "runtime" ? <RuntimeLogManagement /> : <SystemLogManagement />}
  </div>;
}
