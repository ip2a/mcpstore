import { ScrollPane } from "@/components/shared/scroll-pane"
import { OverviewUsageChartCard } from "@/features/overview/overview-usage-chart-card"
import { OverviewUsageHeatmapCard } from "@/features/overview/overview-usage-heatmap-card"
import { useI18n } from "@/lib/i18n-context"

export function OverviewView() {
  const { t } = useI18n()

  return (
    <ScrollPane className="flex-1" innerClassName="flex flex-col gap-3 pb-2">
      <div className="flex items-center gap-2 px-0.5">
        <span className="text-xs text-muted-foreground">{t("homeMockHint")}</span>
      </div>
      <OverviewUsageChartCard />
      <OverviewUsageHeatmapCard />
    </ScrollPane>
  )
}
