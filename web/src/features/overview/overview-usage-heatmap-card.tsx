import { useEffect, useMemo, useRef, useState } from "react"
import { useQuery } from "@tanstack/react-query"

import { Card, CardContent } from "@/components/ui/card"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useI18n } from "@/lib/i18n-context"
import { getFrameActivityStats, type HeatmapDay } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"

const HOURS = 24
const DAYS = 7
/** One cell = one (weekday, hour-of-day) bucket aggregated over the trailing week. */
const FETCH_HOURS = HOURS * DAYS
const REFRESH_MS = 60_000
const DAY_LABEL_COL = "1.75rem"
const STATS_MIN_HEIGHT = 220

const WEEK_KEYS = [
  "homeWeekMon",
  "homeWeekTue",
  "homeWeekWed",
  "homeWeekThu",
  "homeWeekFri",
  "homeWeekSat",
  "homeWeekSun",
] as const

/** Idle → low → mid → high; matches the original homepage heatmap palette. */
const OK_CLASS = [
  "bg-muted",
  "bg-[#e8e7f8]",
  "bg-[#c4c2ef]",
  "bg-[#8b89dc]",
  "bg-[#5b5bd5]",
] as const
const ERROR_CLASS = ["bg-[#f5b8b8]", "bg-[#dc2626]"] as const

type DayCell = {
  count: number
  tool: number
  resource: number
  prompt: number
  other: number
  errors: number
}

type Matrix = {
  cells: DayCell[][]
  requests: number
  errors: number
  maxCalls: number
  maxErrors: number
  activeDays: number
  errorDays: number
}

function emptyCell(): DayCell {
  return { count: 0, tool: 0, resource: 0, prompt: 0, other: 0, errors: 0 }
}

/** Row 0 = Monday; columns = local hours 0–23. Buckets are summed (DST-safe). */
function buildMatrix(days: HeatmapDay[]): Matrix {
  const cells = Array.from({ length: DAYS }, () =>
    Array.from({ length: HOURS }, emptyCell),
  )
  let errors = 0
  let maxCalls = 0
  let maxErrors = 0
  for (const day of days) {
    const date = new Date(day.bucket_unix_ms)
    const cell = cells[(date.getDay() + 6) % 7][date.getHours()]
    cell.tool += day.tool_calls
    cell.resource += day.resource_calls
    cell.prompt += day.prompt_calls
    cell.other += day.other_calls
    cell.errors += day.errors
    cell.count += day.tool_calls + day.resource_calls + day.prompt_calls
    errors += day.errors
  }
  let requests = 0
  let activeDays = 0
  let errorDays = 0
  for (const row of cells) {
    let dayCalls = 0
    let dayErrors = 0
    for (const cell of row) {
      requests += cell.count
      dayCalls += cell.count
      dayErrors += cell.errors
      maxCalls = Math.max(maxCalls, cell.count)
      maxErrors = Math.max(maxErrors, cell.errors)
    }
    if (dayCalls > 0) activeDays++
    if (dayErrors > 0) errorDays++
  }
  return {
    cells,
    requests,
    errors,
    maxCalls,
    maxErrors,
    activeDays,
    errorDays,
  }
}

function shadeClass(cell: DayCell, maxCalls: number, maxErrors: number): string {
  if (cell.errors > 0) {
    return ERROR_CLASS[maxErrors > 0 && cell.errors * 2 > maxErrors ? 1 : 0]
  }
  if (cell.count <= 0 || maxCalls <= 0) return OK_CLASS[0]
  return OK_CLASS[Math.min(4, Math.max(1, Math.ceil((cell.count / maxCalls) * 4)))]
}

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(Math.round(n))
}

function HeatCell({
  cell,
  dayLabel,
  hour,
  maxCalls,
  maxErrors,
  tipTotal,
  famTools,
  famResources,
  famPrompts,
  famOther,
  callsUnit,
  errorsLabel,
}: {
  cell: DayCell
  dayLabel: string
  hour: number
  maxCalls: number
  maxErrors: number
  tipTotal: string
  famTools: string
  famResources: string
  famPrompts: string
  famOther: string
  callsUnit: string
  errorsLabel: string
}) {
  const swatch = (
    <div
      className={cn(
        "min-h-0 min-w-0 rounded-[4px]",
        shadeClass(cell, maxCalls, maxErrors),
      )}
    />
  )

  if (cell.count === 0 && cell.errors === 0) return swatch

  const parts: string[] = []
  if (cell.tool > 0) parts.push(`${famTools} ${cell.tool}`)
  if (cell.resource > 0) parts.push(`${famResources} ${cell.resource}`)
  if (cell.prompt > 0) parts.push(`${famPrompts} ${cell.prompt}`)
  if (cell.other > 0) parts.push(`${famOther} ${cell.other}`)

  return (
    <Tooltip>
      <TooltipTrigger asChild>{swatch}</TooltipTrigger>
      <TooltipContent side="top" sideOffset={6} className="max-w-xs">
        <p className="font-medium">{tipTotal}</p>
        <p className="text-background/75">
          {parts.join(" · ") || `0 ${callsUnit}`}
        </p>
        {cell.errors > 0 ? (
          <p className="mt-0.5 font-medium text-[#ff6b6b]">{errorsLabel}</p>
        ) : null}
        <p className="sr-only">
          {dayLabel} {hour}:00
        </p>
      </TooltipContent>
    </Tooltip>
  )
}

export function OverviewUsageHeatmapCard({ className }: { className?: string }) {
  const { t } = useI18n()
  const { data: stats } = useQuery({
    queryKey: queryKeys.frameActivityStats(FETCH_HOURS, 3_600_000),
    queryFn: () => getFrameActivityStats(FETCH_HOURS, 3_600_000),
    refetchInterval: REFRESH_MS,
  })

  const matrix = useMemo(() => buildMatrix(stats ?? []), [stats])
  const hasTraffic = matrix.requests > 0 || matrix.errors > 0

  const contentRef = useRef<HTMLDivElement>(null)
  const [showStats, setShowStats] = useState(true)

  useEffect(() => {
    const el = contentRef.current
    if (!el) return
    const update = (height: number) => {
      setShowStats((prev) => {
        // Hysteresis: hide under threshold, show again with a little headroom.
        if (prev) return height >= STATS_MIN_HEIGHT
        return height >= STATS_MIN_HEIGHT + 40
      })
    }
    const ro = new ResizeObserver((entries) => {
      const rect = entries[0]?.contentRect
      if (!rect) return
      update(rect.height)
    })
    ro.observe(el)
    update(el.clientHeight)
    return () => ro.disconnect()
  }, [])

  return (
    <Card className={cn("gap-0", className)}>
      <CardContent
        ref={contentRef}
        className="flex min-h-0 flex-1 flex-col px-4 pb-3 pt-3"
      >
        {showStats ? (
          <div className="grid shrink-0 grid-cols-3 gap-6 pb-3">
            <div className="min-w-0">
              <p className="text-xs text-muted-foreground">
                {t("homeHeat24StatRequests")}
              </p>
              <p className="mt-1 text-3xl font-semibold tabular-nums tracking-tight">
                {formatCompact(matrix.requests)}
              </p>
            </div>
            <div className="min-w-0">
              <p className="text-xs text-muted-foreground">
                {t("homeHeat24StatErrors")}
              </p>
              <p
                className={cn(
                  "mt-1 text-3xl font-semibold tabular-nums tracking-tight",
                  matrix.errors > 0 && "text-[#dc2626]",
                )}
              >
                {formatCompact(matrix.errors)}
              </p>
            </div>
            <div className="min-w-0">
              <p className="text-xs text-muted-foreground">
                {t("homeHeat24StatDaily")}
              </p>
              <p className="mt-1 text-3xl font-semibold tabular-nums tracking-tight">
                {formatCompact(matrix.requests / DAYS)}
              </p>
            </div>
          </div>
        ) : null}

        {hasTraffic ? (
          <>
            <div
              className="grid min-h-0 w-full min-w-0 flex-1 gap-[3px]"
              style={{
                gridTemplateColumns: `${DAY_LABEL_COL} repeat(${HOURS}, minmax(0, 1fr))`,
                gridTemplateRows: `auto repeat(${DAYS}, minmax(0, 1fr))`,
              }}
            >
              <div aria-hidden />
              {Array.from({ length: HOURS }, (_, hour) => (
                <div
                  key={`h-${hour}`}
                  className="flex h-4 items-end justify-center text-[10px] leading-none text-muted-foreground"
                >
                  {hour % 6 === 0 ? String(hour).padStart(2, "0") : ""}
                </div>
              ))}

              {WEEK_KEYS.map((weekKey, day) => (
                <div key={weekKey} className="contents">
                  <div className="flex items-center text-[10px] leading-none text-muted-foreground">
                    {t(weekKey)}
                  </div>
                  {Array.from({ length: HOURS }, (_, hour) => {
                    const cell = matrix.cells[day][hour]
                    return (
                      <HeatCell
                        key={`${day}-${hour}`}
                        cell={cell}
                        dayLabel={t(weekKey)}
                        hour={hour}
                        maxCalls={matrix.maxCalls}
                        maxErrors={matrix.maxErrors}
                        tipTotal={t("homeHeat24TipTotal", {
                          label: `${t(weekKey)} ${hour}:00–${hour + 1}:00`,
                          count: cell.count,
                        })}
                        famTools={t("homeFrameFamTools")}
                        famResources={t("homeFrameFamResources")}
                        famPrompts={t("homeFrameFamPrompts")}
                        famOther={t("homeFrameFamOther")}
                        callsUnit={t("homeCallsUnit")}
                        errorsLabel={t("homeFrameStatErrors", {
                          count: cell.errors,
                        })}
                      />
                    )
                  })}
                </div>
              ))}
            </div>

            <div className="mt-2 flex shrink-0 items-center justify-end gap-1.5">
              <span className="text-xs text-muted-foreground">{t("homeLess")}</span>
              {OK_CLASS.map((cls) => (
                <div
                  key={cls}
                  className={cn("size-3 shrink-0 rounded-[4px]", cls)}
                />
              ))}
              <div
                className={cn("size-3 shrink-0 rounded-[4px]", ERROR_CLASS[1])}
              />
              <span className="text-xs text-muted-foreground">{t("homeMore")}</span>
            </div>
          </>
        ) : (
          <p className="flex min-h-0 flex-1 items-center justify-center text-sm text-muted-foreground">
            {t("homeFrameEmpty")}
          </p>
        )}
      </CardContent>
    </Card>
  )
}
