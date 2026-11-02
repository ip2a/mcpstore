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
import { getFrameActivityStats, type HeatmapDay } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

type TimeRange = "7d" | "3d" | "24h" | "1h"
type SplitDim = "service" | "tool" | "resource" | "frame"

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
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
  }
  if (range === "24h") {
    return `${date.getHours().toString().padStart(2, "0")}:00`
  }
  if (range === "3d") {
    return `${date.getMonth() + 1}/${date.getDate()} ${date.getHours().toString().padStart(2, "0")}:00`
  }
  return `${date.getMonth() + 1}/${date.getDate()}`
}

export function OverviewUsageChartCard({ className }: { className?: string }) {
  const { t } = useI18n()
  const [range, setRange] = useState<TimeRange>("24h")
  const [split, setSplit] = useState<SplitDim>("tool")
  
  const { data: stats } = useQuery({
    queryKey: queryKeys.frameActivityStats(TIME_RANGE_HOURS[range]),
    queryFn: () => getFrameActivityStats(TIME_RANGE_HOURS[range]),
    refetchInterval: 60_000,
  })

  // Build series data based on split dimension
  const { series, chartConfig } = useMemo(() => {
    if (!stats || stats.length === 0) {
      return { series: [], chartConfig: {} }
    }

    let seriesData: Array<{ key: string; color: string; total: number }>
    
    switch (split) {
      case "tool":
        seriesData = [
          { key: "Tools", color: SERIES_COLORS[0], total: stats.reduce((sum, s) => sum + s.tool_calls, 0) },
        ]
        break
      case "resource":
        seriesData = [
          { key: "Resources", color: SERIES_COLORS[1], total: stats.reduce((sum, s) => sum + s.resource_calls, 0) },
        ]
        break
      case "service":
        seriesData = [
          { key: "Prompts", color: SERIES_COLORS[2], total: stats.reduce((sum, s) => sum + s.prompt_calls, 0) },
          { key: "Tools", color: SERIES_COLORS[0], total: stats.reduce((sum, s) => sum + s.tool_calls, 0) },
          { key: "Resources", color: SERIES_COLORS[1], total: stats.reduce((sum, s) => sum + s.resource_calls, 0) },
          { key: "Other", color: SERIES_COLORS[3], total: stats.reduce((sum, s) => sum + s.other_calls, 0) },
        ].filter(s => s.total > 0)
        break
      case "frame":
      default:
        seriesData = [
          { key: "Tools", color: SERIES_COLORS[0], total: stats.reduce((sum, s) => sum + s.tool_calls, 0) },
          { key: "Resources", color: SERIES_COLORS[1], total: stats.reduce((sum, s) => sum + s.resource_calls, 0) },
          { key: "Prompts", color: SERIES_COLORS[2], total: stats.reduce((sum, s) => sum + s.prompt_calls, 0) },
          { key: "Other", color: SERIES_COLORS[3], total: stats.reduce((sum, s) => sum + s.other_calls, 0) },
        ].filter(s => s.total > 0)
    }

    const config = Object.fromEntries(
      seriesData.map((s) => [s.key, { label: s.key, color: s.color }]),
    ) satisfies ChartConfig

    return { series: seriesData, chartConfig: config }
  }, [stats, split])

  const chartData = useMemo(() => {
    if (!stats) return []
    
    return stats.map((bucket) => {
      const row: Record<string, number | string> = {
        label: bucketLabel(bucket.bucket_unix_ms, range),
      }
      
      switch (split) {
        case "tool":
          row["Tools"] = bucket.tool_calls
          break
        case "resource":
          row["Resources"] = bucket.resource_calls
          break
        case "service":
          row["Tools"] = bucket.tool_calls
          row["Resources"] = bucket.resource_calls
          row["Prompts"] = bucket.prompt_calls
          row["Other"] = bucket.other_calls
          break
        case "frame":
        default:
          row["Tools"] = bucket.tool_calls
          row["Resources"] = bucket.resource_calls
          row["Prompts"] = bucket.prompt_calls
          row["Other"] = bucket.other_calls
      }
      
      return row
    })
  }, [stats, range, split])

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
            <ToggleGroupItem value="tool">Tool</ToggleGroupItem>
            <ToggleGroupItem value="resource">Resource</ToggleGroupItem>
            <ToggleGroupItem value="frame">Frame</ToggleGroupItem>
          </ToggleGroup>
        </div>
      </div>

      <CardContent className="grid min-h-0 flex-1 gap-4 p-4 lg:grid-cols-[minmax(0,1fr)_280px]">
        <ChartContainer
          config={chartConfig}
          className="aspect-auto min-h-[140px] h-full w-full min-w-0 flex-1"
        >
          <BarChart data={chartData} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
            <CartesianGrid vertical={false} strokeDasharray="3 3" />
            <XAxis
              dataKey="label"
              tickLine={false}
              axisLine={false}
              tickMargin={8}
              interval={Math.max(Math.floor(chartData.length / 12), 0)}
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
