import { useMemo, useState } from "react"
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

type MetricTab = "token" | "cost" | "requests" | "speed"
type SplitDim = "model" | "modelProvider" | "provider" | "agent"

type SeriesKey = "morph" | "claude" | "ccwan"

const SERIES: {
  key: SeriesKey
  label: string
  color: string
  legend: {
    total: number
    requests: number
    cost: string
    speed?: string
    timed?: number
    ttft?: string
    hitRates: string
    failures?: number
  }
}[] = [
  {
    key: "morph",
    label: "morph",
    color: "#5b5bd5",
    legend: {
      total: 104_022_404,
      requests: 1031,
      cost: "$4.41",
      speed: "42 tokens/s",
      timed: 1008,
      ttft: "6.5s",
      hitRates: "93%、96%",
      failures: 10,
    },
  },
  {
    key: "claude",
    label: "claude-opus-5-5",
    color: "#38bdf8",
    legend: {
      total: 4_544_927,
      requests: 44,
      cost: "$4.90",
      speed: "790 tokens/s",
      ttft: "7.7s",
      hitRates: "95%",
      failures: 1,
    },
  },
  {
    key: "ccwan",
    label: "cc-wan/morph",
    color: "#34d399",
    legend: {
      total: 106_246,
      requests: 1,
      cost: "$0.004",
      hitRates: "100%",
    },
  },
]

const HOURS = Array.from({ length: 24 }, (_, hour) => {
  const label = `${String(hour).padStart(2, "0")}:00`
  const spike =
    hour >= 15 && hour <= 17
      ? 1
      : hour === 8 || hour === 11
        ? 0.35
        : hour >= 12 && hour <= 20
          ? 0.12
          : 0
  return {
    label,
    morph: Math.round(spike * (hour === 16 ? 72_000_000 : 18_000_000)),
    claude: Math.round(spike * 2_800_000),
    ccwan: Math.round(spike * 65_000),
  }
})

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(0)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function formatInt(n: number): string {
  return n.toLocaleString("en-US")
}

export function OverviewUsageChartCard({ className }: { className?: string }) {
  const { t } = useI18n()
  const [metric, setMetric] = useState<MetricTab>("token")
  const [split, setSplit] = useState<SplitDim>("model")

  const chartConfig = useMemo(
    () =>
      Object.fromEntries(
        SERIES.map((s) => [s.key, { label: s.label, color: s.color }]),
      ) satisfies ChartConfig,
    [],
  )

  const chartData = useMemo(() => {
    if (metric === "token") return HOURS
    const scale =
      metric === "cost" ? 0.000_000_04 : metric === "requests" ? 0.000_01 : 0.000_000_5
    return HOURS.map((row) => ({
      label: row.label,
      morph: Math.max(1, Math.round(row.morph * scale)),
      claude: Math.max(1, Math.round(row.claude * scale)),
      ccwan: Math.max(0, Math.round(row.ccwan * scale)),
    }))
  }, [metric])

  const yMax = Math.max(
    ...chartData.map((row) => row.morph + row.claude + row.ccwan),
    1,
  )

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
          <ToggleGroupItem value="token" className="px-3">
            Token
          </ToggleGroupItem>
          <ToggleGroupItem value="cost" className="px-3">
            {t("homeMetricCost")}
          </ToggleGroupItem>
          <ToggleGroupItem value="requests" className="px-3">
            {t("homeLlmCalls")}
          </ToggleGroupItem>
          <ToggleGroupItem value="speed" className="px-3">
            {t("homeMetricSpeed")}
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
            <ToggleGroupItem value="model">{t("homeDimModel")}</ToggleGroupItem>
            <ToggleGroupItem value="modelProvider">{t("homeDimModelProvider")}</ToggleGroupItem>
            <ToggleGroupItem value="provider">{t("homeDimProvider")}</ToggleGroupItem>
            <ToggleGroupItem value="agent">Agent</ToggleGroupItem>
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
              tickFormatter={(v) => formatCompact(Number(v))}
              domain={[0, Math.ceil(yMax * 1.05)]}
              tick={{ fill: "var(--muted-foreground)", fontSize: 11 }}
            />
            <ChartTooltip cursor={{ fill: "var(--muted)" }} content={<ChartTooltipContent />} />
            {SERIES.map((s) => (
              <Bar
                key={s.key}
                dataKey={s.key}
                stackId="usage"
                fill={s.color}
                radius={s.key === "ccwan" ? [4, 4, 0, 0] : 0}
              />
            ))}
          </BarChart>
        </ChartContainer>

        <aside className="flex min-w-0 flex-col gap-4 border-t pt-4 lg:border-t-0 lg:border-l lg:pt-0 lg:pl-4">
          {SERIES.map((s) => (
            <div key={s.key} className="min-w-0 space-y-1">
              <div className="flex items-center gap-2">
                <span className="size-2 shrink-0 rounded-full" style={{ background: s.color }} />
                <span className="truncate text-sm font-medium">{s.label}</span>
              </div>
              <div className="text-lg font-semibold tabular-nums tracking-tight">
                {formatInt(s.legend.total)}
              </div>
              <p className="text-xs leading-relaxed text-muted-foreground">
                {formatInt(s.legend.requests)} {t("homeCallsUnit")}
                {" · "}
                {t("homeAboutCost", { amount: s.legend.cost })}
                {s.legend.speed ? (
                  <>
                    {" · "}
                    {s.legend.speed}
                  </>
                ) : null}
                {s.legend.timed ? (
                  <>
                    {" · "}
                    {formatInt(s.legend.timed)} {t("homeTimedCalls")}
                  </>
                ) : null}
                {s.legend.ttft ? (
                  <>
                    {" · "}
                    {t("homeTtft", { sec: s.legend.ttft })}
                  </>
                ) : null}
                {" · "}
                {t("homeHitRateShort")} {s.legend.hitRates}
                {s.legend.failures ? (
                  <>
                    {" · "}
                    <span className="text-destructive">
                      {t("homeFailures", { count: s.legend.failures })}
                    </span>
                  </>
                ) : null}
              </p>
              {split !== "model" ? (
                <p className="text-[11px] text-muted-foreground/80">{t("homeMockHint")}</p>
              ) : null}
            </div>
          ))}
        </aside>
      </CardContent>
    </Card>
  )
}
