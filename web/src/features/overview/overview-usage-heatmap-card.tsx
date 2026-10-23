import { useMemo } from "react"

import { Card, CardContent } from "@/components/ui/card"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

const WEEKS = 53
const DAY_LABELS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const
const DAY_LABEL_COL = "1.75rem"
const TOTAL_DAYS = WEEKS * DAY_LABELS.length

function cellLevel(week: number, day: number): number {
  const seed = week * 7 + day
  const wave = Math.sin(seed * 0.17) * 0.5 + 0.5
  const ramp = 0.35 + 0.65 * (week / (WEEKS - 1))
  const dayBias = day <= 4 ? 1 : 0.72
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

const LEVEL_LABEL_KEY = [
  "homeHeatLevelIdle",
  "homeHeatLevelLow",
  "homeHeatLevelMid",
  "homeHeatLevelHigh",
  "homeHeatLevelPeak",
] as const

const TOKEN_BY_LEVEL = [0, 420, 1_800, 4_200, 9_500] as const

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

const HEATMAP_GRID_TEMPLATE = `${DAY_LABEL_COL} repeat(${WEEKS}, minmax(0, 1fr))`

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(Math.round(n))
}

export function OverviewUsageHeatmapCard({ className }: { className?: string }) {
  const { t } = useI18n()

  const grid = useMemo(() => {
    return DAY_LABELS.map((_, day) =>
      Array.from({ length: WEEKS }, (_, week) => cellLevel(week, day)),
    )
  }, [])

  const { totalTokens, activeDays, levelCounts } = useMemo(() => {
    let tokens = 0
    let active = 0
    const counts = [0, 0, 0, 0, 0]
    for (let day = 0; day < grid.length; day++) {
      for (let week = 0; week < grid[day].length; week++) {
        const level = grid[day][week]
        counts[level]++
        tokens += TOKEN_BY_LEVEL[level]
        if (level > 0) active++
      }
    }
    return { totalTokens: tokens, activeDays: active, levelCounts: counts }
  }, [grid])

  const activeRate = Math.round((activeDays / TOTAL_DAYS) * 100)

  const dayLabel = (day: number) => {
    if (day === 0) return t("homeWeekMon")
    if (day === 2) return t("homeWeekWed")
    if (day === 4) return t("homeWeekFri")
    if (day === 6) return t("homeWeekSun")
    return ""
  }

  return (
    <Card className={cn("gap-0", className)}>
      <CardContent className="flex min-h-0 flex-1 flex-col px-4 pb-4 pt-4">
        <div className="grid shrink-0 grid-cols-2 gap-8 pb-4">
          <div className="min-w-0">
            <p className="text-xs text-muted-foreground">{t("homeTokens")}</p>
            <p className="mt-1 flex flex-wrap items-baseline gap-x-1.5 gap-y-0">
              <span className="text-3xl font-semibold tabular-nums tracking-tight">
                {formatCompact(totalTokens)}
              </span>
              <span className="text-sm text-muted-foreground">token</span>
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              {t("homeUsageHeatmapWeeks", { weeks: WEEKS })}
            </p>
          </div>
          <div className="min-w-0">
            <p className="text-xs text-muted-foreground">{t("homeHeatmapActiveDays")}</p>
            <p className="mt-1 flex flex-wrap items-baseline gap-x-1.5 gap-y-0">
              <span className="text-3xl font-semibold tabular-nums tracking-tight">
                {activeRate}%
              </span>
              <span className="text-sm text-muted-foreground">{t("homeHeatmapActive")}</span>
            </p>
            <div
              className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-muted"
              role="progressbar"
              aria-valuenow={activeRate}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              <div
                className="h-full rounded-full bg-[#5b5bd5] transition-[width]"
                style={{ width: `${activeRate}%` }}
              />
            </div>
            <p className="mt-1 text-xs text-muted-foreground">
              {t("homeHeatmapDaysOf", { active: activeDays, total: TOTAL_DAYS })}
            </p>
          </div>
        </div>

        <div
          className="grid min-h-0 w-full min-w-0 flex-1 gap-[3px]"
          style={{
            gridTemplateColumns: HEATMAP_GRID_TEMPLATE,
            gridTemplateRows: `auto repeat(${DAY_LABELS.length}, auto)`,
          }}
        >
          <div aria-hidden className="h-4" />
          <div
            className="relative col-start-2 h-4"
            style={{ gridColumn: `2 / span ${WEEKS}` }}
          >
            {MONTH_MARKERS.map((m) => (
              <span
                key={`${m.week}-${m.label}`}
                className="absolute text-[10px] leading-none text-muted-foreground"
                style={{ left: `${(m.week / WEEKS) * 100}%` }}
              >
                {m.label}
              </span>
            ))}
          </div>

          {DAY_LABELS.map((_, day) => (
            <div key={day} className="contents">
              <div className="flex items-center text-[10px] leading-none text-muted-foreground">
                {dayLabel(day)}
              </div>
              {Array.from({ length: WEEKS }, (_, week) => (
                <div
                  key={week}
                  title={`${dayLabel(day) || DAY_LABELS[day]} · W${week + 1}`}
                  className={cn(
                    "aspect-square w-full rounded-[4px]",
                    LEVEL_CLASS[grid[day][week]],
                  )}
                />
              ))}
            </div>
          ))}
        </div>

        <div className="mt-3 flex shrink-0 flex-wrap items-center justify-start gap-x-4 gap-y-2">
          {LEVEL_CLASS.map((cls, level) => (
            <div key={level} className="flex items-center gap-1.5">
              <div className={cn("size-3 shrink-0 rounded-[4px]", cls)} />
              <span className="text-xs text-muted-foreground">
                {t(LEVEL_LABEL_KEY[level])}
              </span>
              <span className="text-xs tabular-nums text-foreground/80">
                {t("homeHeatLegendDays", { count: levelCounts[level] })}
              </span>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  )
}
