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
import { getUsageStats, type UsageStats, type UsageSeriesPoint } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

type MetricTab = "calls" | "errors" | "avg" | "max"
type SplitDim = "caller" | "service" | "tool" | "instance"

const SERIES_COLORS = ["#5b5bd5", "#38bdf8", "#34d399", "#f59e0b"]
const DIM_SERIES_KEY: Record<SplitDim, keyof UsageStats> = {
  caller: "by_caller",
  service: "by_service",
  tool: "by_tool",
  instance: "by_instance",
}

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(0)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function formatMs(ms: number): string {
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`
  return `${Math.round(ms)}ms`
}

function bucketLabel(bucketUnixMs: number): string {
  const hour = new Date(bucketUnixMs).getHours().toString().padStart(2, "0")
  return `${hour}:00`
}

export function OverviewUsageChartCard({ className }: { className?: string }) {
  const { t } = useI18n()
  const [metric, setMetric] = useState<MetricTab>("calls")
  const [split, setSplit] = useState<SplitDim>("caller")
  const { data: stats } = useQuery({
    queryKey: queryKeys.usageStats,
    queryFn: () => getUsageStats(24),
    refetchInterval: 60_000,
  })

  // Top-4 series of the selected dimension, ranked by total calls in the window.
  const series = useMemo(() => {
    const rows = (stats?.[DIM_SERIES_KEY[split]] ?? []) as UsageSeriesPoint[]
    const totals = new Map<string, { calls: number; errors: number; weightedMs: number }>()
    for (const row of rows) {
      const agg = totals.get(row.label) ?? { calls: 0, errors: 0, weightedMs: 0 }
      agg.calls += row.calls
      agg.errors += row.errors
      agg.weightedMs += row.avg_ms * row.calls
      totals.set(row.label, agg)
    }
    return [...totals.entries()]
      .sort((left, right) => right[1].calls - left[1].calls)
      .slice(0, 4)
      .map(([label, agg], index) => ({
        key: label === "" ? "—" : label,
        color: SERIES_COLORS[index % SERIES_COLORS.length],
        calls: agg.calls,
        errors: agg.errors,
        avgMs: agg.calls > 0 ? agg.weightedMs / agg.calls : 0,
      }))
  }, [stats, split])

  const chartConfig = useMemo(
    () =>
      Object.fromEntries(
        series.map((s) => [s.key, { label: s.key, color: s.color }]),
      ) satisfies ChartConfig,
    [series],
  )

  const chartData = useMemo(() => {
    const rows = (stats?.[DIM_SERIES_KEY[split]] ?? []) as UsageSeriesPoint[]
    const active = new Set(series.map((s) => s.key))
    const buckets = new Set<number>()
    for (const point of stats?.by_hour ?? []) buckets.add(point.bucket_unix_ms)
    for (const point of rows) buckets.add(point.bucket_unix_ms)
    const byBucket = new Map<number, Record<string, number>>(
      [...buckets].sort((a, b) => a - b).map((bucket) => [bucket, {}]),
    )
    for (const point of rows) {
      const key = point.label === "" ? "—" : point.label
      if (!active.has(key)) continue
      const row = byBucket.get(point.bucket_unix_ms)
      if (!row) continue
      row[key] =
        metric === "calls"
          ? point.calls
          : metric === "errors"
            ? point.errors
            : metric === "avg"
              ? point.avg_ms
              : point.max_ms
    }
    return [...byBucket.entries()].map(([bucket, row]) => ({
      label: bucketLabel(bucket),
      ...row,
    }))
  }, [stats, split, metric, series])

  const yMax = Math.max(
    ...chartData.map((row) =>
      series.reduce((sum, s) => sum + ((row[s.key] as number | undefined) ?? 0), 0),
    ),
    1,
  )
  const totalCalls = series.reduce((sum, s) => sum + s.calls, 0)
  const valueFormatter = metric === "avg" || metric === "max" ? formatMs : formatCompact

  return (
    <Card className={cn("gap-0 py-0", className)}>
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b px-4 py-3">
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          value={metric}
          onValueChange={(v) => v && setMetric(v as MetricTab)}
        >
          <ToggleGroupItem value="calls" className="px-3">
            {t("homeMetricCalls")}
          </ToggleGroupItem>
          <ToggleGroupItem value="errors" className="px-3">
            {t("homeMetricErrors")}
          </ToggleGroupItem>
          <ToggleGroupItem value="avg" className="px-3">
            {t("homeMetricAvgMs")}
          </ToggleGroupItem>
          <ToggleGroupItem value="max" className="px-3">
            {t("homeMetricMaxMs")}
          </ToggleGroupItem>
        </ToggleGroup>

        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-muted-foreground">{t("homeSplitDimension")}</span>
          <ToggleGroup
            type="single"
            variant="outline"
            size="sm"
            value={split}
            onValueChange={(v) => v && setSplit(v as SplitDim)}
          >
            <ToggleGroupItem value="caller">{t("homeDimCaller")}</ToggleGroupItem>
            <ToggleGroupItem value="service">{t("homeDimService")}</ToggleGroupItem>
            <ToggleGroupItem value="tool">{t("homeDimTool")}</ToggleGroupItem>
            <ToggleGroupItem value="instance">{t("homeDimInstance")}</ToggleGroupItem>
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
              interval={3}
              tick={{ fill: "var(--muted-foreground)", fontSize: 11 }}
            />
            <YAxis
              tickLine={false}
              axisLine={false}
              width={44}
              tickFormatter={(v) => valueFormatter(Number(v))}
              domain={[0, Math.ceil(yMax * 1.05)]}
              tick={{ fill: "var(--muted-foreground)", fontSize: 11 }}
            />
            <ChartTooltip cursor={{ fill: "var(--muted)" }} content={<ChartTooltipContent />} />
            {series.map((s) => (
              <Bar
                key={s.key}
                dataKey={s.key}
                stackId="usage"
                fill={s.color}
                radius={s.key === series[series.length - 1]?.key ? [4, 4, 0, 0] : 0}
              />
            ))}
          </BarChart>
        </ChartContainer>

        <aside className="flex min-w-0 flex-col gap-4 border-t pt-4 lg:border-t-0 lg:border-l lg:pt-0 lg:pl-4">
          {totalCalls === 0 ? (
            <p className="text-sm leading-relaxed text-muted-foreground">{t("homeNoCalls")}</p>
          ) : (
            series.map((s) => (
              <div key={s.key} className="min-w-0 space-y-1">
                <div className="flex items-center gap-2">
                  <span className="size-2 shrink-0 rounded-full" style={{ background: s.color }} />
                  <span className="truncate font-mono text-sm font-medium">{s.key}</span>
                </div>
                <div className="text-lg font-semibold tabular-nums tracking-tight">
                  {formatCompact(s.calls)}
                </div>
                <p className="text-xs leading-relaxed text-muted-foreground">
                  {t("homeCallsUnit")} {formatCompact(s.calls)}
                  {" · "}
                  <span className={s.errors > 0 ? "text-destructive" : undefined}>
                    {t("homeFailures", { count: s.errors })}
                  </span>
                  {" · "}
                  {t("homeMetricAvgMs")} {formatMs(s.avgMs)}
                </p>
              </div>
            ))
          )}
        </aside>
      </CardContent>
    </Card>
  )
}
