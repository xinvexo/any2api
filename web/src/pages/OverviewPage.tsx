import { SystemOverview } from "@/features/system-status";

export function OverviewPage() {
  return (
    <div className="space-y-7">
      <header>
        <p className="text-sm font-medium text-accent-copy">实例</p>
        <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">总览</h1>
      </header>
      <SystemOverview />
    </div>
  );
}
