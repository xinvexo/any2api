import { AffinityOverview } from "@/features/affinity";
import { BalancingOverview } from "@/features/balancing";
import { OverviewUsageSection } from "@/features/overview-usage";
import { SystemOverview } from "@/features/system-status";

export function OverviewPage() {
  return (
    <div className="min-w-0">
      <SystemOverview />
      <OverviewUsageSection />
      <div className="grid border-t border-subtle lg:grid-cols-[minmax(0,1.65fr)_minmax(17rem,0.7fr)] lg:divide-x lg:divide-subtle">
        <BalancingOverview />
        <AffinityOverview />
      </div>
    </div>
  );
}
