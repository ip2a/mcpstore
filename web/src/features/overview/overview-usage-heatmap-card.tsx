import { useMemo } from "react"
import { useQuery } from "@tanstack/react-query"

import { Card, CardContent } from "@/components/ui/card"
import { useI18n } from "@/lib/i18n-context"
import { getHeatmapStats, type HeatmapDay } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"

const WEEKS = 53
const DAY_ROWS = 7 // Mon..Sun
const DAY_MS = 86_400_000
const DAY_LABELS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const
const DAY_LABEL_COL = "1.75rem"
const TOTAL_CELLS = WEEKS * DAY_ROWS

// 正常：靛蓝色系按当日调用总量分档；异常：红色系按当日错误数分档。
const OK_CLASS = ["bg-muted", "bg-[#e8e7f8]", "bg-[#c4c2ef]", "bg-[#8b89dc]", "bg-[#5b5bd5]"]
const ERROR_CLASS = ["bg-[#f5b8b8]", "bg-[#dc2626]"]
const LEVEL_LABEL_KEY = [
  "homeHeatLevelIdle",
  "homeHeatLevelLow",
  "homeHeatLevelMid",
  "homeHeatLevelHigh",
  "homeHeatLevelError",
] as const

const HEATMAP_GRID_TEMPLATE = `${DAY_LABEL_COL} repeat(${WEEKS}, minmax(0, 1fr))`

/** UTC-day bucket → grid cell: per-primitive call counts + error count. */
type DayCell = {
  bucketUnixMs: number
  date: string // "M/D"
  month: number
  calls: number
  day: HeatmapDay
  future: boolean
}

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(Math.round(n))
}

/** Columns of Mon..Sun weeks ending with the current (partial) week; UTC-day aligned. */
function buildWeeks(days: HeatmapDay[]): DayCell[][] {
  const byBucket = new Map(days.map((day) => [day.bucket_unix_ms, day]))
  const nowBucket = Math.floor(Date.now() / DAY_MS) * DAY_MS
  const weekday = (new Date(nowBucket).getUTCDay() + 6) % 7 // 0=Mon
  const thisMonday = nowBucket - weekday * DAY_MS
  const cell = (bucketUnixMs: number): DayCell => {
    const found = byBucket.get(bucketUnixMs)
    const utc = new Date(bucketUnixMs)
    return {
      bucketUnixMs,
      date: `${utc.getUTCMonth() + 1}/${utc.getUTCDate()}`,
      month: utc.getUTCMonth(),
      calls:
        found === undefined
          ? 0
          : found.tool_calls + found.resource_calls + found.prompt_calls + found.other_calls,
      day: found ?? {
        bucket_unix_ms: bucketUnixMs,
        tool_calls: 0,
        resource_calls: 0,
        prompt_calls: 0,
        other_calls: 0,
        errors: 0,
      },
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

function shadeClass(cell: DayCell, maxCalls: number, maxErrors: number): string {
  if (cell.future) return "bg-muted/40"
  if (cell.day.errors > 0) {
    return ERROR_CLASS[cell.day.errors * 2 > maxErrors ? 1 : 0]
  }
  if (cell.calls <= 0 || maxCalls <= 0) return OK_CLASS[0]
  return OK_CLASS[Math.min(3, Math.max(1, Math.ceil((cell.calls / maxCalls) * 3)))]
}

function cellTitle(cell: DayCell): string {
  const parts: string[] = [cell.date]
  if (cell.day.tool_calls > 0) parts.push(`tools ${cell.day.tool_calls}`)
  if (cell.day.resource_calls > 0) parts.push(`resources ${cell.day.resource_calls}`)
  if (cell.day.prompt_calls > 0) parts.push(`prompts ${cell.day.prompt_calls}`)
  if (cell.day.other_calls > 0) parts.push(`other ${cell.day.other_calls}`)
  if (cell.day.errors > 0) parts.push(`⚠ ${cell.day.errors}`)
  return parts.join(" · ")
}

export function OverviewUsageHeatmapCard({ className }: { className?: string }) {
  const { t } = useI18n()
  const { data: daily } = useQuery({
    queryKey: queryKeys.heatmapStats,
    queryFn: () => getHeatmapStats(371),
    refetchInterval: 60_000,
  })

  const weeks = useMemo(() => buildWeeks(daily ?? []), [daily])

  const { totalCalls, totalErrors, activeDays, errorDays, pastCells, levelCounts, maxCalls, maxErrors, monthMarkers } =
    useMemo(() => {
      let calls = 0
      let errors = 0
      let active = 0
      let bad = 0
      let past = 0
      let maxC = 0
      let maxE = 0
      const counts = [0, 0, 0, 0, 0]
      for (let day = 0; day < weeks.length; day++) {
        for (let week = 0; week < weeks[day].length; week++) {
          const cell = weeks[day][week]
          if (cell.future) continue
          past++
          calls += cell.calls
          errors += cell.day.errors
          if (cell.calls > 0) active++
          if (cell.day.errors > 0) bad++
          maxC = Math.max(maxC, cell.calls)
          maxE = Math.max(maxE, cell.day.errors)
        }
      }
      for (let day = 0; day < weeks.length; day++) {
        for (let week = 0; week < weeks[day].length; week++) {
          const cell = weeks[day][week]
          const level =
            !cell.future && cell.day.errors > 0 ? 4 : cell.calls > 0 ? Math.min(3, Math.max(1, Math.ceil((cell.calls / (maxC || 1)) * 3))) : 0
          counts[level]++
        }
      }
      // Month marker per column: label on the first column whose Monday starts a new month.
      const markers: { week: number; month: number }[] = []
      let seenMonth = -1
      for (let week = 0; week < WEEKS; week++) {
        const mondayMonth = weeks[0][week].month
        if (mondayMonth !== seenMonth) {
          markers.push({ week, month: mondayMonth })
          seenMonth = mondayMonth
        }
      }
      return {
        totalCalls: calls,
        totalErrors: errors,
        activeDays: active,
        errorDays: bad,
        pastCells: past,
        levelCounts: counts,
        maxCalls: maxC,
        maxErrors: maxE,
        monthMarkers: markers,
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
            <p className="text-xs text-muted-foreground">{t("homeHeatmapErrors")}</p>
            <p className="mt-1 flex flex-wrap items-baseline gap-x-1.5 gap-y-0">
              <span
                className={cn(
                  "text-3xl font-semibold tabular-nums tracking-tight",
                  totalErrors > 0 && "text-[#dc2626]",
                )}
              >
                {formatCompact(totalErrors)}
              </span>
              <span className="text-sm text-muted-foreground">{t("homeHeatmapErrorsUnit")}</span>
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
              {errorDays > 0 ? ` · ${t("homeHeatmapErrorDays", { count: errorDays })}` : ""}
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
            {monthMarkers.map((m) => (
              <span
                key={`${m.week}-${m.month}`}
                className="absolute text-[10px] leading-none text-muted-foreground"
                style={{ left: `${(m.week / WEEKS) * 100}%` }}
              >
                {t("homeHeatmapMonthLabel", { month: m.month + 1 })}
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
                return (
                  <div
                    key={week}
                    title={cellTitle(cell)}
                    className={cn(
                      "aspect-square w-full rounded-[4px]",
                      shadeClass(cell, maxCalls, maxErrors),
                    )}
                  />
                )
              })}
            </div>
          ))}
        </div>

        <div className="mt-3 flex shrink-0 flex-wrap items-center justify-start gap-x-4 gap-y-2">
          {LEVEL_LABEL_KEY.map((key, level) => {
            const cls =
              level === LEVEL_LABEL_KEY.length - 1 ? ERROR_CLASS[1] : OK_CLASS[level]
            return (
              <div key={level} className="flex items-center gap-1.5">
                <div className={cn("size-3 shrink-0 rounded-[4px]", cls)} />
                <span className="text-xs text-muted-foreground">{t(key)}</span>
                <span className="text-xs tabular-nums text-foreground/80">
                  {t("homeHeatLegendDays", { count: levelCounts[level] })}
                </span>
              </div>
            )
          })}
        </div>
      </CardContent>
    </Card>
  )
}
