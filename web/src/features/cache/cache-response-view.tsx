import type { ReactNode } from "react"
import { ActivityIcon, PackageIcon } from "lucide-react"
import { Bar, BarChart, Cell, Label, Pie, PieChart, XAxis, YAxis } from "recharts"

import { CacheCollectionDetail } from "@/features/cache/cache-layer-browser"
import {
  LAYER_I18N_KEY,
  type CacheCollectionNode,
  type CacheLayerId,
  type CacheTree,
} from "@/features/cache/cache-model"
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import {
  ChartContainer,
  ChartLegend,
  ChartLegendContent,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart"
import { SectionHeading } from "@/components/shared/section-heading"
import { useI18n } from "@/lib/i18n-context"

const HIT_MISS_COLORS = {
  hits: "var(--chart-2)",
  misses: "var(--chart-3)",
  errors: "var(--chart-5)",
} as const

const LAYER_COLORS: Record<CacheLayerId, string> = {
  entity: "var(--chart-1)",
  relations: "var(--chart-2)",
  state: "var(--chart-3)",
  event: "var(--chart-4)",
}

function asNumber(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0
}

function formatHitRate(value: unknown): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "—"
  const pct = value <= 1 ? value * 100 : value
  return `${pct.toFixed(pct >= 10 ? 0 : 1)}%`
}

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function ChartPanel({
  title,
  children,
}: {
  title: string
  children: ReactNode
}) {
  return (
    <section className="flex min-h-0 min-w-0 flex-1 flex-col gap-3 rounded-md border border-border/50 p-4">
      <SectionHeading title={title} titleAs="h2" className="shrink-0 border-b-0 pb-0" />
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">{children}</div>
    </section>
  )
}

function ChartEmpty({ title, description, icon: Icon }: { title: string; description: string; icon: typeof ActivityIcon }) {
  return (
    <Empty className="min-h-[220px] flex-1 border-0 p-4 md:p-6">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <Icon />
        </EmptyMedia>
        <EmptyTitle>{title}</EmptyTitle>
        <EmptyDescription>{description}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  )
}

function RequestQualityChart({ metrics }: { metrics: Record<string, unknown> }) {
  const { t } = useI18n()
  const hits = asNumber(metrics.hits)
  const misses = asNumber(metrics.misses)
  const errors = asNumber(metrics.errors)
  const total = hits + misses + errors
  const hitRate = formatHitRate(metrics.hit_rate)
  const latency = asNumber(metrics.avg_latency_ms)

  const chartConfig = {
    hits: { label: t("cacheHits"), color: HIT_MISS_COLORS.hits },
    misses: { label: t("cacheMisses"), color: HIT_MISS_COLORS.misses },
    errors: { label: t("cacheErrors"), color: HIT_MISS_COLORS.errors },
  } satisfies ChartConfig

  const data = [
    { key: "hits", value: hits, fill: HIT_MISS_COLORS.hits },
    { key: "misses", value: misses, fill: HIT_MISS_COLORS.misses },
    { key: "errors", value: errors, fill: HIT_MISS_COLORS.errors },
  ].filter((row) => row.value > 0)

  if (total <= 0) {
    return (
      <ChartEmpty
        title={t("cacheRequestQuality")}
        description={t("cacheNoRequestMetrics")}
        icon={ActivityIcon}
      />
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <ChartContainer config={chartConfig} className="mx-auto aspect-square h-full max-h-[280px] w-full">
        <PieChart>
          <ChartTooltip cursor={false} content={<ChartTooltipContent nameKey="key" hideLabel />} />
          <Pie
            data={data}
            dataKey="value"
            nameKey="key"
            innerRadius="58%"
            outerRadius="78%"
            strokeWidth={2}
            isAnimationActive={false}
          >
            {data.map((row) => (
              <Cell key={row.key} fill={row.fill} />
            ))}
            <Label
              content={({ viewBox }) => {
                if (!viewBox || !("cx" in viewBox) || !("cy" in viewBox)) return null
                return (
                  <text x={viewBox.cx} y={viewBox.cy} textAnchor="middle" dominantBaseline="middle">
                    <tspan
                      x={viewBox.cx}
                      y={(viewBox.cy ?? 0) - 6}
                      className="fill-foreground text-2xl font-semibold tabular-nums"
                    >
                      {hitRate}
                    </tspan>
                    <tspan
                      x={viewBox.cx}
                      y={(viewBox.cy ?? 0) + 16}
                      className="fill-muted-foreground text-xs"
                    >
                      {t("cacheHitRate")}
                    </tspan>
                  </text>
                )
              }}
            />
          </Pie>
          <ChartLegend content={<ChartLegendContent nameKey="key" />} />
        </PieChart>
      </ChartContainer>
      {latency > 0 ? (
        <p className="shrink-0 text-center text-xs text-muted-foreground">
          {t("cacheAvgLatencyMs", { ms: Math.round(latency) })}
        </p>
      ) : null}
    </div>
  )
}

function KeysByLayerChart({ tree }: { tree: CacheTree }) {
  const { t } = useI18n()
  const rows = tree.layers.map((layer) => ({
    layer: layer.layer,
    label: t(LAYER_I18N_KEY[layer.layer]),
    keys: layer.keyCount,
    types: layer.typeCount,
    fill: LAYER_COLORS[layer.layer],
  }))
  const hasKeys = rows.some((row) => row.keys > 0)

  const chartConfig = Object.fromEntries(
    rows.map((row) => [row.layer, { label: row.label, color: row.fill }]),
  ) satisfies ChartConfig

  if (!hasKeys) {
    return (
      <ChartEmpty
        title={t("cacheKeysByLayer")}
        description={t("cacheNoKeysToChart")}
        icon={PackageIcon}
      />
    )
  }

  const yMax = Math.max(...rows.map((row) => row.keys), 1)

  return (
    <ChartContainer config={chartConfig} className="aspect-auto h-full min-h-[220px] w-full">
      <BarChart
        accessibilityLayer
        data={rows}
        layout="vertical"
        margin={{ top: 4, right: 12, left: 4, bottom: 4 }}
      >
        <XAxis
          type="number"
          tickLine={false}
          axisLine={false}
          tickFormatter={(v) => formatCompact(Number(v))}
          domain={[0, Math.ceil(yMax * 1.05)]}
          tick={{ fontSize: 11 }}
        />
        <YAxis
          type="category"
          dataKey="label"
          tickLine={false}
          axisLine={false}
          width={88}
          tick={{ fontSize: 12 }}
        />
        <ChartTooltip
          cursor={{ fill: "var(--muted)" }}
          content={
            <ChartTooltipContent
              labelKey="label"
              formatter={(value, _name, item) => {
                const types = asNumber(item?.payload?.types)
                return (
                  <div className="flex flex-1 items-center justify-between gap-4 leading-none">
                    <div className="grid gap-0.5">
                      <span className="text-muted-foreground">{t("cacheTotalKeys")}</span>
                      <span className="text-muted-foreground">
                        {t("typesCount", { count: types })}
                      </span>
                    </div>
                    <span className="font-mono font-medium tabular-nums">
                      {typeof value === "number" ? value.toLocaleString() : String(value)}
                    </span>
                  </div>
                )
              }}
            />
          }
        />
        <Bar dataKey="keys" radius={4} isAnimationActive={false}>
          {rows.map((row) => (
            <Cell key={row.layer} fill={row.fill} />
          ))}
        </Bar>
      </BarChart>
    </ChartContainer>
  )
}

export function CacheOverviewContent({ tree }: { tree: CacheTree }) {
  const { t } = useI18n()

  return (
    <div className="@container grid min-h-0 min-w-0 flex-1 gap-4 @min-[42rem]:grid-cols-2">
      <ChartPanel title={t("cacheRequestQuality")}>
        {tree.requestMetrics ? (
          <RequestQualityChart metrics={tree.requestMetrics} />
        ) : (
          <ChartEmpty
            title={t("cacheRequestQuality")}
            description={t("cacheNoRequestMetrics")}
            icon={ActivityIcon}
          />
        )}
      </ChartPanel>
      <ChartPanel title={t("cacheKeysByLayer")}>
        <KeysByLayerChart tree={tree} />
      </ChartPanel>
    </div>
  )
}

export function CacheLayerContent({ collection }: { collection: CacheCollectionNode | null }) {
  const { t } = useI18n()

  if (!collection) {
    return <p className="text-sm text-muted-foreground">{t("cacheSelectCollectionHint")}</p>
  }

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
      <CacheCollectionDetail collection={collection} />
    </div>
  )
}
