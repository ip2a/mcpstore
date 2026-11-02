import { useMemo, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import {
  Bar,
  BarChart,
  CartesianGrid,
  XAxis,
  YAxis,
} from "recharts"

import {
  Card,
  CardContent,
} from "@/components/ui/card"
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"
import { getFrameActivityStats, getFrameActivityByService } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

type TimeRange = "7d" | "3d" | "24h" | "1h"
type SplitDim = "service" | "primitive" | "traces"

const SERIES_COLORS = ["#5b5bd5", "#38bdf8", "#34d399", "#f59e0b"]

const TIME_RANGE_HOURS: Record<TimeRange, number> = {
  "7d": 24 * 7,
  "3d": 24 * 3,
  "24h": 24,
  "1h": 1,
}

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(0)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function bucketLabel(bucketUnixMs: number, range: TimeRange): string {
  const date = new Date(bucketUnixMs)
  if (range === "1h") {
    // 1 hour: show HH:MM
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
  }
  if (range === "24h") {
    // 24 hours: show HH:00
    return `${date.getHours().toString().padStart(2, "0")}:00`
  }
  if (range === "3d") {
    // 3 days: show MM/DD HH:00, but only show date change points prominently
    const hours = date.getHours()
    if (hours === 0 || hours === 12) {
      return `${date.getMonth() + 1}/${date.getDate()} ${hours.toString().padStart(2, "0")}:00`
    }
    return `${hours.toString().padStart(2, "0")}:00`
  }
  // 7 days: show MM/DD
  return `${date.getMonth() + 1}/${date.getDate()}`
}

export function OverviewUsageChartCard({ className }: { className?: string }) {
  const { t } = useI18n()
  const [range, setRange] = useState<TimeRange>("24h")
  const [split, setSplit] = useState<SplitDim>("primitive")
  
  const { data: stats } = useQuery({
    queryKey: queryKeys.frameActivityStats(TIME_RANGE_HOURS[range]),
    queryFn: () => getFrameActivityStats(TIME_RANGE_HOURS[range]),
    refetchInterval: 60_000,
  })

  const { data: serviceStats } = useQuery({
    queryKey: queryKeys.frameActivityByService(TIME_RANGE_HOURS[range]),
    queryFn: () => getFrameActivityByService(TIME_RANGE_HOURS[range]),
    refetchInterval: 60_000,
    enabled: split === "service",
  })

  // Build series data based on split dimension
  const { series, chartConfig } = useMemo(() => {
    if (split === "service") {
      if (!serviceStats || serviceStats.length === 0) {
        return { series: [], chartConfig: {} }
      }
      
      // Group by service and sum totals
      const serviceTotals = new Map<string, number>()
      for (const point of serviceStats) {
        const current = serviceTotals.get(point.service) ?? 0
        serviceTotals.set(point.service, current + point.frame_count)
      }
      
      // Top 4 services by total frame count
      const topServices = [...serviceTotals.entries()]
        .sort((a, b) => b[1] - a[1])
        .slice(0, 4)
        .map(([service, total], idx) => ({
          key: service,
          color: SERIES_COLORS[idx % SERIES_COLORS.length],
          total,
        }))
      
      const config = Object.fromEntries(
        topServices.map((s) => [s.key, { label: s.key, color: s.color }]),
      ) satisfies ChartConfig
      
      return { series: topServices, chartConfig: config }
    }
    
    if (!stats || stats.length === 0) {
      return { series: [], chartConfig: {} }
    }

    let seriesData: Array<{ key: string; color: string; total: number }>
    
    switch (split) {
      case "primitive":
        // MCP 三大原语
        seriesData = [
          { key: "Tools", color: SERIES_COLORS[0], total: stats.reduce((sum, s) => sum + s.tool_calls, 0) },
          { key: "Resources", color: SERIES_COLORS[1], total: stats.reduce((sum, s) => sum + s.resource_calls, 0) },
          { key: "Prompts", color: SERIES_COLORS[2], total: stats.reduce((sum, s) => sum + s.prompt_calls, 0) },
        ].filter(s => s.total > 0)
        break
      case "traces":
        // 所有帧类型（包括 Other: ping, initialize 等）
        seriesData = [
          { key: "Tools", color: SERIES_COLORS[0], total: stats.reduce((sum, s) => sum + s.tool_calls, 0) },
          { key: "Resources", color: SERIES_COLORS[1], total: stats.reduce((sum, s) => sum + s.resource_calls, 0) },
          { key: "Prompts", color: SERIES_COLORS[2], total: stats.reduce((sum, s) => sum + s.prompt_calls, 0) },
          { key: "Other", color: SERIES_COLORS[3], total: stats.reduce((sum, s) => sum + s.other_calls, 0) },
        ].filter(s => s.total > 0)
        break
      default:
        seriesData = []
    }

    const config = Object.fromEntries(
      seriesData.map((s) => [s.key, { label: s.key, color: s.color }]),
    ) satisfies ChartConfig

    return { series: seriesData, chartConfig: config }
  }, [stats, serviceStats, split])

  const chartData = useMemo(() => {
    if (split === "service") {
      if (!serviceStats) return []
      
      // Build chart data with service columns
      const buckets = new Map<number, Record<string, number>>()
      const activeServices = new Set(series.map((s) => s.key))
      
      for (const point of serviceStats) {
        if (!activeServices.has(point.service)) continue
        
        const bucket = buckets.get(point.bucket_unix_ms) ?? {}
        bucket[point.service] = (bucket[point.service] ?? 0) + point.frame_count
        buckets.set(point.bucket_unix_ms, bucket)
      }
      
      return [...buckets.entries()]
        .sort((a, b) => a[0] - b[0])
        .map(([bucketMs, data]) => ({
          label: bucketLabel(bucketMs, range),
          ...data,
        }))
    }
    
    if (!stats) return []
    
    return stats.map((bucket) => {
      const row: Record<string, number | string> = {
        label: bucketLabel(bucket.bucket_unix_ms, range),
      }
      
      switch (split) {
        case "primitive":
          // 三大原语
          row["Tools"] = bucket.tool_calls
          row["Resources"] = bucket.resource_calls
          row["Prompts"] = bucket.prompt_calls
          break
        case "traces":
          // 所有帧类型
          row["Tools"] = bucket.tool_calls
          row["Resources"] = bucket.resource_calls
          row["Prompts"] = bucket.prompt_calls
          row["Other"] = bucket.other_calls
          break
      }
      
      return row
    })
  }, [stats, serviceStats, range, split, series])

  const yMax = Math.max(
    ...chartData.map((row) =>
      series.reduce((sum, s) => sum + ((row[s.key] as number | undefined) ?? 0), 0),
    ),
    1,
  )
  const totalCalls = series.reduce((sum, s) => sum + s.total, 0)

  return (
    <Card className={cn("gap-0 py-0", className)}>
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b px-4 py-3">
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          value={range}
          onValueChange={(v) => v && setRange(v as TimeRange)}
        >
          <ToggleGroupItem value="7d" className="px-3">
            7 Days
          </ToggleGroupItem>
          <ToggleGroupItem value="3d" className="px-3">
            3 Days
          </ToggleGroupItem>
          <ToggleGroupItem value="24h" className="px-3">
            24 Hours
          </ToggleGroupItem>
          <ToggleGroupItem value="1h" className="px-3">
            1 Hour
          </ToggleGroupItem>
        </ToggleGroup>

        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-muted-foreground">Split by</span>
          <ToggleGroup
            type="single"
            variant="outline"
            size="sm"
            value={split}
            onValueChange={(v) => v && setSplit(v as SplitDim)}
          >
            <ToggleGroupItem value="service">Service</ToggleGroupItem>
            <ToggleGroupItem value="primitive">Primitive</ToggleGroupItem>
            <ToggleGroupItem value="traces">Traces</ToggleGroupItem>
          </ToggleGroup>
        </div>
      </div>

      <CardContent className="grid min-h-0 flex-1 gap-4 p-4 lg:grid-cols-[minmax(0,1fr)_280px]">
        <ChartContainer
          config={chartConfig}
          className="aspect-auto min-h-[240px] h-full w-full min-w-0 flex-1"
        >
          <BarChart 
            data={chartData} 
            margin={{ top: 8, right: 8, left: 0, bottom: range === "3d" ? 36 : 24 }}
            barCategoryGap="10%"
            barGap={0}
          >
            <CartesianGrid vertical={false} strokeDasharray="3 3" />
            <XAxis
              dataKey="label"
              tickLine={false}
              axisLine={false}
              tickMargin={8}
              height={32}
              angle={range === "3d" ? -15 : 0}
              textAnchor={range === "3d" ? "end" : "middle"}
              interval={(() => {
                // Adaptive tick interval based on data density and time range
                const len = chartData.length
                if (range === "1h") {
                  // 60 data points: show every 5th (12 ticks)
                  return 4
                }
                if (range === "24h") {
                  // 24 data points: show every 2nd (12 ticks)
                  return 1
                }
                if (range === "3d") {
                  // 72 data points: show every 6th (12 ticks)
                  return 5
                }
                if (range === "7d") {
                  // 7 data points: show all
                  return 0
                }
                // Fallback: aim for ~12 ticks
                if (len <= 12) return 0
                return Math.floor(len / 12)
              })()}
              tick={{ fill: "var(--muted-foreground)", fontSize: 11 }}
            />
            <YAxis
              tickLine={false}
              axisLine={false}
              width={44}
              tickFormatter={(v) => formatCompact(Number(v))}
              domain={[0, Math.ceil(yMax * 1.05)]}
              tick={{ fill: "var(--muted-foreground)", fontSize: 11 }}
            />
            <ChartTooltip cursor={{ fill: "var(--muted)" }} content={<ChartTooltipContent />} />
            {series.map((s, idx) => (
              <Bar
                key={s.key}
                dataKey={s.key}
                stackId="usage"
                fill={s.color}
                radius={idx === series.length - 1 ? [4, 4, 0, 0] : 0}
              />
            ))}
          </BarChart>
        </ChartContainer>

        <aside className="flex min-w-0 flex-col gap-4 border-t pt-4 lg:border-t-0 lg:border-l lg:pt-0 lg:pl-4">
          {totalCalls === 0 ? (
            <p className="text-sm leading-relaxed text-muted-foreground">No activity in this period</p>
          ) : (
            series.map((s) => (
              <div key={s.key} className="min-w-0 space-y-1">
                <div className="flex items-center gap-2">
                  <span className="size-2 shrink-0 rounded-full" style={{ background: s.color }} />
                  <span className="truncate font-mono text-sm font-medium">{s.key}</span>
                </div>
                <div className="text-lg font-semibold tabular-nums tracking-tight">
                  {formatCompact(s.total)}
                </div>
                <p className="text-xs leading-relaxed text-muted-foreground">
                  {s.total} {s.total === 1 ? "call" : "calls"}
                </p>
              </div>
            ))
          )}
        </aside>
      </CardContent>
    </Card>
  )
}
