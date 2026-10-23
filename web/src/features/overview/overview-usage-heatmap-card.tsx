import { useMemo, useState } from "react"

import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

const WEEKS = 53
const DAY_LABELS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const

type HeatMetric = "token" | "requests" | "cost"

function cellLevel(week: number, day: number, metric: HeatMetric): number {
  if (week < 32) return 0
  const seed = (week * 7 + day) * (metric === "cost" ? 3 : metric === "requests" ? 5 : 1)
  const wave = Math.sin(seed * 0.17) * 0.5 + 0.5
  const ramp = (week - 32) / (WEEKS - 32)
  const dayBias = day <= 4 ? 1 : 0.65
  const raw = wave * ramp * dayBias
  if (raw < 0.08) return 0
  if (raw < 0.22) return 1
  if (raw < 0.4) return 2
  if (raw < 0.62) return 3
  return 4
}

const LEVEL_CLASS = [
  "bg-muted",
  "bg-[#e8e7f8]",
  "bg-[#c4c2ef]",
  "bg-[#8b89dc]",
  "bg-[#5b5bd5]",
] as const

const MONTH_MARKERS = [
  { week: 0, label: "11月" },
  { week: 5, label: "12月" },
  { week: 9, label: "1月" },
  { week: 14, label: "2月" },
  { week: 18, label: "3月" },
  { week: 23, label: "4月" },
  { week: 27, label: "5月" },
  { week: 32, label: "6月" },
  { week: 36, label: "7月" },
  { week: 41, label: "8月" },
  { week: 45, label: "9月" },
  { week: 49, label: "10月" },
]

export function OverviewUsageHeatmapCard({ className }: { className?: string }) {
  const { t } = useI18n()
  const [metric, setMetric] = useState<HeatMetric>("token")

  const grid = useMemo(() => {
    return DAY_LABELS.map((_, day) =>
      Array.from({ length: WEEKS }, (_, week) => cellLevel(week, day, metric)),
    )
  }, [metric])

  const dayLabel = (day: number) => {
    if (day === 0) return t("homeWeekMon")
    if (day === 2) return t("homeWeekWed")
    if (day === 4) return t("homeWeekFri")
    if (day === 6) return t("homeWeekSun")
    return ""
  }

  return (
    <Card className={cn("gap-3", className)}>
      <CardHeader className="flex flex-row flex-wrap items-start justify-between gap-3 space-y-0 pb-0">
        <div>
          <CardTitle className="text-base">{t("homeUsageHeatmap")}</CardTitle>
          <p className="mt-1 text-xs text-muted-foreground">
            {t("homeUsageHeatmapWeeks", { weeks: WEEKS })}
          </p>
        </div>
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          value={metric}
          onValueChange={(v) => v && setMetric(v as HeatMetric)}
        >
          <ToggleGroupItem value="token">Token</ToggleGroupItem>
          <ToggleGroupItem value="requests">{t("homeLlmCalls")}</ToggleGroupItem>
          <ToggleGroupItem value="cost">{t("homeMetricCost")}</ToggleGroupItem>
        </ToggleGroup>
      </CardHeader>

      <CardContent className="overflow-x-auto pb-2">
        <div className="min-w-[720px]">
          <div className="relative mb-1 h-4 pl-8">
            {MONTH_MARKERS.map((m) => (
              <span
                key={`${m.week}-${m.label}`}
                className="absolute text-[10px] text-muted-foreground"
                style={{ left: `calc(2rem + ${m.week} * 13px)` }}
              >
                {m.label}
              </span>
            ))}
          </div>
          <div className="flex gap-1">
            <div className="flex w-7 shrink-0 flex-col gap-[3px] pt-[2px] text-[10px] text-muted-foreground">
              {DAY_LABELS.map((_, day) => (
                <div key={day} className="flex h-[11px] items-center leading-none">
                  {dayLabel(day)}
                </div>
              ))}
            </div>
            <div className="flex gap-[3px]">
              {Array.from({ length: WEEKS }, (_, week) => (
                <div key={week} className="flex flex-col gap-[3px]">
                  {grid.map((row, day) => (
                    <div
                      key={day}
                      title={`${dayLabel(day) || DAY_LABELS[day]} · W${week + 1}`}
                      className={cn("size-[11px] rounded-[2px]", LEVEL_CLASS[row[week]])}
                    />
                  ))}
                </div>
              ))}
            </div>
          </div>
          <div className="mt-3 flex items-center justify-end gap-1.5 text-[10px] text-muted-foreground">
            <span>{t("homeLess")}</span>
            {LEVEL_CLASS.map((cls, i) => (
              <div key={i} className={cn("size-[11px] rounded-[2px]", cls)} />
            ))}
            <span>{t("homeMore")}</span>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
