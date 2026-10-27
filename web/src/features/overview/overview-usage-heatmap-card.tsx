import { useMemo } from "react"
import { useQuery } from "@tanstack/react-query"

import { Card, CardContent } from "@/components/ui/card"
import { useI18n } from "@/lib/i18n-context"
import { getDailyUsageStats } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"

const WEEKS = 5
const DAY_ROWS = 7 // Mon..Sun
const DAY_MS = 86_400_000
const DAY_LABELS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const
const DAY_LABEL_COL = "1.75rem"
const TOTAL_CELLS = WEEKS * DAY_ROWS

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

const HEATMAP_GRID_TEMPLATE = `${DAY_LABEL_COL} repeat(${WEEKS}, minmax(0, 1fr))`

/** UTC-day bucket → { calls, errors } for one calendar day. */
type DayCell = {
  bucketUnixMs: number
  date: string // "M/D"
  calls: number
  errors: number
  future: boolean
}

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(Math.round(n))
}

/** Columns of Mon..Sun weeks ending with the current (partial) week; UTC-day aligned. */
function buildWeeks(days: { bucket_unix_ms: number; calls: number; errors: number }[]): DayCell[][] {
  const byBucket = new Map(days.map((day) => [day.bucket_unix_ms, day]))
  const nowBucket = Math.floor(Date.now() / DAY_MS) * DAY_MS
  const weekday = (new Date(nowBucket).getUTCDay() + 6) % 7 // 0=Mon
  const thisMonday = nowBucket - weekday * DAY_MS
  const cell = (bucketUnixMs: number): DayCell => {
    const found = byBucket.get(bucketUnixMs)
    const date = new Date(bucketUnixMs)
    return {
      bucketUnixMs,
      date: `${date.getUTCMonth() + 1}/${date.getUTCDate()}`,
      calls: found?.calls ?? 0,
      errors: found?.errors ?? 0,
      future: bucketUnixMs > nowBucket,
    }
  }
  return Array.from({ length: DAY_ROWS }, (_, day) =>
    Array.from({ length: WEEKS }, (_, week) => {
      const monday = thisMonday - (WEEKS - 1 - week) * 7 * DAY_MS
      return cell(monday + day * DAY_MS)
    }),
  )
}

function cellLevel(calls: number, maxCalls: number): number {
  if (calls <= 0) return 0
  if (maxCalls <= 0) return 0
  return Math.min(4, Math.max(1, Math.ceil((calls / maxCalls) * 4)))
}

export function OverviewUsageHeatmapCard({ className }: { className?: string }) {
  const { t } = useI18n()
  const { data: daily } = useQuery({
    queryKey: queryKeys.dailyUsageStats,
    queryFn: () => getDailyUsageStats(35),
    refetchInterval: 60_000,
  })

  const weeks = useMemo(() => buildWeeks(daily ?? []), [daily])

  const { totalCalls, activeDays, pastCells, levelCounts, maxCalls, mondayLabels } = useMemo(() => {
    let calls = 0
    let active = 0
    let past = 0
    let max = 0
    const counts = [0, 0, 0, 0, 0]
    for (let day = 0; day < weeks.length; day++) {
      for (let week = 0; week < weeks[day].length; week++) {
        if (weeks[day][week].future) continue
        past++
        calls += weeks[day][week].calls
        if (weeks[day][week].calls > 0) active++
        max = Math.max(max, weeks[day][week].calls)
      }
    }
    for (let day = 0; day < weeks.length; day++) {
      for (let week = 0; week < weeks[day].length; week++) {
        counts[cellLevel(weeks[day][week].calls, max)]++
      }
    }
    const labels = weeks[0].map((cell) => cell.date)
    return {
      totalCalls: calls,
      activeDays: active,
      pastCells: past,
      levelCounts: counts,
      maxCalls: max,
      mondayLabels: labels,
    }
  }, [weeks])

  const activeRate = pastCells > 0 ? Math.round((activeDays / pastCells) * 100) : 0

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
            <p className="text-xs text-muted-foreground">{t("homeHeatmapCalls")}</p>
            <p className="mt-1 flex flex-wrap items-baseline gap-x-1.5 gap-y-0">
              <span className="text-3xl font-semibold tabular-nums tracking-tight">
                {formatCompact(totalCalls)}
              </span>
              <span className="text-sm text-muted-foreground">calls</span>
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
              {t("homeHeatmapDaysOf", { active: activeDays, total: pastCells || TOTAL_CELLS })}
            </p>
          </div>
        </div>

        <div
          className="grid min-h-0 w-full min-w-0 flex-1 gap-[3px]"
          style={{
            gridTemplateColumns: HEATMAP_GRID_TEMPLATE,
            gridTemplateRows: `auto repeat(${DAY_ROWS}, auto)`,
          }}
        >
          <div aria-hidden className="h-4" />
          <div
            className="relative col-start-2 h-4"
            style={{ gridColumn: `2 / span ${WEEKS}` }}
          >
            {mondayLabels.map((label, week) => (
              <span
                key={`${week}-${label}`}
                className="absolute text-[10px] leading-none text-muted-foreground"
                style={{ left: `${(week / WEEKS) * 100}%` }}
              >
                {label}
              </span>
            ))}
          </div>

          {DAY_LABELS.map((_, day) => (
            <div key={day} className="contents">
              <div className="flex items-center text-[10px] leading-none text-muted-foreground">
                {dayLabel(day)}
              </div>
              {Array.from({ length: WEEKS }, (_, week) => {
                const cell = weeks[day][week]
                const level = cellLevel(cell.calls, maxCalls)
                return (
                  <div
                    key={week}
                    title={t("homeHeatmapCellTitle", {
                      date: cell.date,
                      calls: cell.future ? 0 : cell.calls,
                    })}
                    className={cn(
                      "aspect-square w-full rounded-[4px]",
                      cell.future ? "bg-muted/40" : LEVEL_CLASS[level],
                    )}
                  />
                )
              })}
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
