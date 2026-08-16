import { OverviewUsageSection } from "@/features/overview-usage";
import { SystemOverview } from "@/features/system-status";

export function OverviewPage() {
  return (
    <div className="mx-auto flex w-full max-w-[1440px] min-w-0 flex-col gap-8 pb-4 sm:gap-10 sm:pb-6">
      <SystemOverview />
      <OverviewUsageSection />
    </div>
  );
}
