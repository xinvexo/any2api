import { AffinityOverview } from "@/features/affinity";
import { BalancingOverview } from "@/features/balancing";
import { SystemOverview } from "@/features/system-status";

export function OverviewPage() {
  return (
    <div className="space-y-5">
      <SystemOverview />
      <BalancingOverview />
      <AffinityOverview />
    </div>
  );
}
