import { OverviewUsageChartCard } from "@/features/overview/overview-usage-chart-card"
import { OverviewUsageHeatmapCard } from "@/features/overview/overview-usage-heatmap-card"
import { cn } from "@/lib/utils"

const panelCardClass =
  "@container flex min-h-0 flex-col overflow-hidden rounded-lg border border-border/50 bg-card shadow-xs"

export function OverviewView() {
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
      <OverviewUsageChartCard
        className={cn(panelCardClass, "min-h-0 flex-[1.15] gap-0 py-0 shadow-xs")}
      />
      <OverviewUsageHeatmapCard
        className={cn(panelCardClass, "min-h-0 flex-1 gap-0 py-0 shadow-xs")}
      />
    </div>
  )
}
