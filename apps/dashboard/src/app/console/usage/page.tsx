import { FreePlanBanner } from "@/components/free-plan-banner";
import { UsageChartCard } from "@/components/usage-chart-card";
import { UsageEvents } from "@/components/usage-events";
import { UsageMetricCards } from "@/components/usage-metric-cards";
import { UsageCards, UsageExtraCards } from "@/components/usage-cards";

export default function UsagePage() {
  return (
    <div className="mx-auto w-full max-w-5xl">
      <h1 className="text-2xl font-medium text-primary">Usage</h1>
      <div className="mt-6 flex flex-col gap-4">
        <UsageCards />
        <UsageExtraCards />
        <FreePlanBanner />
        <UsageMetricCards />
        <UsageChartCard />
        <UsageEvents />
      </div>
    </div>
  );
}
