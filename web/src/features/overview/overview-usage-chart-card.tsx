import { useMemo, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import {
  Area,
  AreaChart,
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
import { cn } from "@/lib/utils"
import {
  getFrameActivityStats,
  getFrameActivityByMethod,
  getFrameActivityByService,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

type TimeRange = "7d" | "24h" | "1h"
type SplitDim = "service" | "primitive" | "method"
type ChartKind = "bar" | "area"

/** Cool blue categorical palette. Identity colors are assigned once per
 * series key and never reshuffled when the visible set changes. */
const SERIES_COLORS = [
  "oklch(0.52 0.16 250)", // deep blue
  "oklch(0.62 0.14 230)", // sky blue
  "oklch(0.58 0.12 210)", // cyan-blue
  "oklch(0.48 0.12 265)", // indigo
  "oklch(0.68 0.10 220)", // soft steel
  "oklch(0.45 0.10 245)", // slate blue
]
const colorByKey = new Map<string, string>()
function colorFor(key: string): string {
  let color = colorByKey.get(key)
  if (!color) {
    color = SERIES_COLORS[colorByKey.size % SERIES_COLORS.length]
    colorByKey.set(key, color)
  }
  return color
}
// Fixed identities first so the primitives keep their colors regardless of
// which split renders first.
for (const key of ["Tools", "Resources", "Prompts"]) colorFor(key)

const TIME_RANGE_HOURS: Record<TimeRange, number> = {
  "7d": 24 * 7,
  "24h": 24,
  "1h": 1,
}

/** Bucket size requested from the backend — mirrors its range-based default. */
const TIME_RANGE_BUCKET_MS: Record<TimeRange, number> = {
  "7d": 86_400_000,
  "24h": 3_600_000,
  "1h": 60_000,
}

type Row = { label: string } & Record<string, number | string>

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(0)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function bucketLabel(bucketMs: number, range: TimeRange): string {
  const date = new Date(bucketMs)
  const hh = (h: number) => h.toString().padStart(2, "0")
  if (range === "1h") {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
  }
  const day = `${date.getMonth() + 1}/${date.getDate()}`
  if (range === "7d") return day
  return `${day} ${hh(date.getHours())}:00`
}

/** Densify the backend's sparse (non-empty-only) buckets into a full timeline
 * so switching ranges never collapses or distorts the x axis. */
function densify(
  points: Array<{ bucket_unix_ms: number; key: string; count: number }>,
  keys: string[],
  range: TimeRange,
): Row[] {
  const bucketMs = TIME_RANGE_BUCKET_MS[range]
  const byBucket = new Map<number, Record<string, number>>()
  for (const point of points) {
    if (!keys.includes(point.key)) continue
    const row = byBucket.get(point.bucket_unix_ms) ?? Object.fromEntries(keys.map((k) => [k, 0]))
    row[point.key] = (row[point.key] as number) + point.count
    byBucket.set(point.bucket_unix_ms, row)
  }
  const first =
    Math.floor((Date.now() - TIME_RANGE_HOURS[range] * 3_600_000) / bucketMs) * bucketMs
  const last = Math.floor(Date.now() / bucketMs) * bucketMs
  const rows: Row[] = []
  for (let bucket = first; bucket <= last; bucket += bucketMs) {
    rows.push({
      label: bucketLabel(bucket, range),
      ...(byBucket.get(bucket) ?? Object.fromEntries(keys.map((k) => [k, 0]))),
    })
  }
  return rows
}

export function OverviewUsageChartCard({ className }: { className?: string }) {
  const [range, setRange] = useState<TimeRange>("24h")
  const [split, setSplit] = useState<SplitDim>("primitive")
  const [kind, setKind] = useState<ChartKind>("bar")

  const hours = TIME_RANGE_HOURS[range]
  const bucketMs = TIME_RANGE_BUCKET_MS[range]

  const { data: stats } = useQuery({
    queryKey: queryKeys.frameActivityStats(hours, bucketMs),
    queryFn: () => getFrameActivityStats(hours, bucketMs),
    refetchInterval: 60_000,
  })
  const { data: serviceStats } = useQuery({
    queryKey: queryKeys.frameActivityByService(hours, bucketMs),
    queryFn: () => getFrameActivityByService(hours, bucketMs),
    refetchInterval: 60_000,
    enabled: split === "service",
  })
  const { data: methodStats } = useQuery({
    queryKey: queryKeys.frameActivityByMethod(hours, bucketMs),
    queryFn: () => getFrameActivityByMethod(hours, bucketMs),
    refetchInterval: 60_000,
    enabled: split === "method",
  })

  const { series, rows } = useMemo(() => {
    if (split === "service") {
      if (!serviceStats) return { series: [], rows: [] }
      const totals = new Map<string, number>()
      for (const point of serviceStats) {
        totals.set(point.service, (totals.get(point.service) ?? 0) + point.frame_count)
      }
      const keys = [...totals.entries()]
        .sort((a, b) => b[1] - a[1])
        .slice(0, 4)
        .map(([key]) => key)
      const series = keys.map((key) => ({ key, color: colorFor(key), total: totals.get(key) ?? 0 }))
      const rows = densify(
        serviceStats.map((p) => ({ bucket_unix_ms: p.bucket_unix_ms, key: p.service, count: p.frame_count })),
        keys,
        range,
      )
      return { series, rows }
    }

    if (split === "method") {
      if (!methodStats) return { series: [], rows: [] }
      const totals = new Map<string, number>()
      for (const point of methodStats) {
        totals.set(point.method, (totals.get(point.method) ?? 0) + point.frame_count)
      }
      const keys = [...totals.entries()]
        .sort((a, b) => b[1] - a[1])
        .slice(0, 6)
        .map(([key]) => key)
      const series = keys.map((key) => ({ key, color: colorFor(key), total: totals.get(key) ?? 0 }))
      const rows = densify(
        methodStats.map((p) => ({ bucket_unix_ms: p.bucket_unix_ms, key: p.method, count: p.frame_count })),
        keys,
        range,
      )
      return { series, rows }
    }

    if (!stats) return { series: [], rows: [] }
    const keys = ["Tools", "Resources", "Prompts"]
    const values: Record<string, number> = { Tools: 0, Resources: 0, Prompts: 0 }
    const points: Array<{ bucket_unix_ms: number; key: string; count: number }> = []
    for (const bucket of stats) {
      points.push(
        { bucket_unix_ms: bucket.bucket_unix_ms, key: "Tools", count: bucket.tool_calls },
        { bucket_unix_ms: bucket.bucket_unix_ms, key: "Resources", count: bucket.resource_calls },
        { bucket_unix_ms: bucket.bucket_unix_ms, key: "Prompts", count: bucket.prompt_calls },
      )
      values.Tools += bucket.tool_calls
      values.Resources += bucket.resource_calls
      values.Prompts += bucket.prompt_calls
    }
    const series = keys.map((key) => ({ key, color: colorFor(key), total: values[key] }))
    return { series, rows: densify(points, keys, range) }
  }, [stats, serviceStats, methodStats, split, range])

  const totalCalls = series.reduce((sum, s) => sum + s.total, 0)

  // Safe data keys let the official ChartStyle component generate CSS color
  // variables even when service or method names contain spaces and slashes.
  const chartSeries = series.map((s, index) => ({
    ...s,
    dataKey: `series${index}`,
    colorVar: `var(--color-series${index})`,
  }))
  const chartRows = rows.map((row): Row => ({
    label: row.label,
    ...Object.fromEntries(
      chartSeries.map((s) => [s.dataKey, (row[s.key] as number) ?? 0]),
    ),
  }))
  const chartConfig = Object.fromEntries(
    chartSeries.map((s) => [
      s.dataKey,
      { label: s.key, color: s.color },
    ]),
  ) satisfies ChartConfig

  // Both bar and stacked area share a summed y domain so the axis stays comparable.
  const yMax = Math.max(
    ...chartRows.map((row) =>
      chartSeries.reduce((sum, s) => sum + (row[s.dataKey] as number), 0),
    ),
    1,
  )

  // Aggressive interval to prevent overlap: skip more labels on dense ranges
  const xInterval = range === "1h" ? 5 : range === "24h" ? 3 : 2

  return (
    <Card className={cn("gap-0 py-0", className)}>
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b px-4 py-3">
        <div className="flex flex-wrap items-center gap-2">
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
            <ToggleGroupItem value="24h" className="px-3">
              24 Hours
            </ToggleGroupItem>
            <ToggleGroupItem value="1h" className="px-3">
              1 Hour
            </ToggleGroupItem>
          </ToggleGroup>
          <ToggleGroup
            type="single"
            variant="outline"
            size="sm"
            value={kind}
            onValueChange={(v) => v && setKind(v as ChartKind)}
          >
            <ToggleGroupItem value="bar" className="px-3">
              Bar
            </ToggleGroupItem>
            <ToggleGroupItem value="area" className="px-3">
              Area
            </ToggleGroupItem>
          </ToggleGroup>
        </div>

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
            <ToggleGroupItem value="method">Method</ToggleGroupItem>
          </ToggleGroup>
        </div>
      </div>

      <CardContent className="grid min-h-0 flex-1 gap-4 p-4 lg:grid-cols-[minmax(0,1fr)_280px]">
        <ChartContainer
          config={chartConfig}
          className="aspect-auto h-[280px] w-full min-w-0"
        >
          {/* Remount per view state: Recharts data-swap animation can leave
              half-rendered charts when toggling range/split. */}
          {kind === "bar" ? (
            <BarChart
              key={`${range}-${split}-bar`}
              accessibilityLayer
              data={chartRows}
              margin={{ top: 8, right: 12, left: 0, bottom: 4 }}
              barCategoryGap="10%"
              barGap={0}
            >
              <CartesianGrid vertical={false} />
              <XAxis
                dataKey="label"
                tickLine={false}
                axisLine={false}
                tickMargin={10}
                height={32}
                interval={xInterval}
                minTickGap={30}
                tick={{ fontSize: 10 }}
              />
              <YAxis
                tickLine={false}
                axisLine={false}
                width={44}
                tickFormatter={(v) => formatCompact(Number(v))}
                domain={[0, Math.ceil(yMax * 1.05)]}
                tick={{ fontSize: 11 }}
              />
              <ChartTooltip
                cursor={{ fill: "var(--muted)" }}
                content={<ChartTooltipContent />}
              />
              {chartSeries.map((s) => (
                <Bar
                  key={s.dataKey}
                  dataKey={s.dataKey}
                  stackId="usage"
                  fill={s.colorVar}
                  isAnimationActive={false}
                  radius={0}
                />
              ))}
            </BarChart>
          ) : (
            <AreaChart
              key={`${range}-${split}-area`}
              accessibilityLayer
              data={chartRows}
              margin={{ top: 8, right: 12, left: 0, bottom: 4 }}
            >
              <CartesianGrid vertical={false} />
              <XAxis
                dataKey="label"
                tickLine={false}
                axisLine={false}
                tickMargin={10}
                height={32}
                interval={xInterval}
                minTickGap={30}
                tick={{ fontSize: 10 }}
              />
              <YAxis
                tickLine={false}
                axisLine={false}
                width={44}
                tickFormatter={(v) => formatCompact(Number(v))}
                domain={[0, Math.ceil(yMax * 1.05)]}
                tick={{ fontSize: 11 }}
              />
              <ChartTooltip
                cursor={false}
                content={<ChartTooltipContent indicator="dot" />}
              />
              {chartSeries.map((s) => (
                <Area
                  key={s.dataKey}
                  dataKey={s.dataKey}
                  type="natural"
                  fill={s.colorVar}
                  fillOpacity={0.4}
                  stroke={s.colorVar}
                  stackId="usage"
                  isAnimationActive={false}
                />
              ))}
            </AreaChart>
          )}
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
                  {s.total} {split === "primitive" ? (s.total === 1 ? "call" : "calls") : "frames"}
                </p>
              </div>
            ))
          )}
        </aside>
      </CardContent>
    </Card>
  )
}
