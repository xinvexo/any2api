import { BalancingOverview } from "@/features/balancing";
import { OverviewUsageSection } from "@/features/overview-usage";
import { SystemOverview } from "@/features/system-status";

export function OverviewPage() {
  return (
    <div className="flex min-w-0 flex-col gap-8 sm:gap-10">
      <SystemOverview />
      <OverviewUsageSection />
      <BalancingOverview />
    </div>
  );
}
