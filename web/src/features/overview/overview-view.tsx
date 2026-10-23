import { useState, type ComponentProps, type ReactNode } from "react"
import {
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  PolarAngleAxis,
  RadialBar,
  RadialBarChart,
  XAxis,
  YAxis,
} from "recharts"
import { InfoIcon, RefreshCwIcon } from "lucide-react"

import { ScrollPane } from "@/components/shared/scroll-pane"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useI18n } from "@/lib/i18n-context"

// TODO(api): every figure below is sample data — swap the datasets, card
// values and gauge for the stats API once the backend exposes usage metrics.
const MOCK = {
  hitRate: 52.41,
  callsTotal: "2.1K",
  callsOk: "1.9K",
  callsErr: "145",
  tokensTotal: "282M",
  tokensPrompt: "280M",
  valueTotal: "$790.59",
  valueCacheRw: "$73.53",
}

type Range = "10m" | "1h" | "1d" | "1w" | "1mo" | "custom"

type Bucket = {
  label: string
  calls: number
  tokens: number
}

function bucket(label: string, calls: number): Bucket {
  return { label, calls, tokens: Number((calls * 0.07 + 0.3).toFixed(1)) }
}

const DATASETS: Record<Exclude<Range, "custom">, Bucket[]> = {
  "10m": [
    bucket("14:51", 2), bucket("14:52", 0), bucket("14:53", 5), bucket("14:54", 0),
    bucket("14:55", 8), bucket("14:56", 0), bucket("14:57", 3), bucket("14:58", 12),
    bucket("14:59", 4), bucket("15:00", 6),
  ],
  "1h": [
    bucket("14:05", 4), bucket("14:10", 0), bucket("14:15", 9), bucket("14:20", 2),
    bucket("14:25", 0), bucket("14:30", 14), bucket("14:35", 6), bucket("14:40", 0),
    bucket("14:45", 11), bucket("14:50", 7), bucket("14:55", 3), bucket("15:00", 8),
  ],
  "1d": [
    bucket("0:00", 0), bucket("1:00", 0), bucket("2:00", 0), bucket("3:00", 0),
    bucket("4:00", 0), bucket("5:00", 2), bucket("6:00", 5), bucket("7:00", 12),
    bucket("8:00", 28), bucket("9:00", 41), bucket("10:00", 36), bucket("11:00", 30),
    bucket("12:00", 22), bucket("13:00", 27), bucket("14:00", 35), bucket("15:00", 44),
    bucket("16:00", 39), bucket("17:00", 25), bucket("18:00", 18), bucket("19:00", 21),
    bucket("20:00", 33), bucket("21:00", 26), bucket("22:00", 9), bucket("23:00", 3),
  ],
  "1w": [
    bucket("10/16", 186), bucket("10/17", 240), bucket("10/18", 431), bucket("10/19", 208),
    bucket("10/20", 312), bucket("10/21", 275), bucket("10/22", 368),
  ],
  "1mo": [
    bucket("9/23", 0), bucket("9/24", 64), bucket("9/25", 0), bucket("9/26", 0),
    bucket("9/27", 0), bucket("9/28", 0), bucket("9/29", 0), bucket("9/30", 0),
    bucket("10/1", 0), bucket("10/2", 0), bucket("10/3", 0), bucket("10/4", 0),
    bucket("10/5", 0), bucket("10/6", 0), bucket("10/7", 0), bucket("10/8", 112),
    bucket("10/9", 480), bucket("10/10", 1536), bucket("10/11", 640), bucket("10/12", 416),
    bucket("10/13", 96), bucket("10/14", 0), bucket("10/15", 144), bucket("10/16", 0),
    bucket("10/17", 0), bucket("10/18", 0), bucket("10/19", 0), bucket("10/20", 0),
    bucket("10/21", 176), bucket("10/22", 192),
  ],
}

const RANGE_ITEMS: { labelKey: string; value: Range }[] = [
  { labelKey: "homeRange1h", value: "1h" },
  { labelKey: "homeRange1d", value: "1d" },
  { labelKey: "homeRange10m", value: "10m" },
  { labelKey: "homeRange1w", value: "1w" },
  { labelKey: "homeRange1mo", value: "1mo" },
  { labelKey: "homeRangeCustom", value: "custom" },
]

// Tooltip rows for the muted "no data" track segment are noise — keep only
// the calls series.
function CallsTooltip({ payload, ...props }: ComponentProps<typeof ChartTooltipContent>) {
  return (
    <ChartTooltipContent
      {...props}
      payload={payload?.filter((item) => item.dataKey === "calls")}
    />
  )
}

export function OverviewView() {
  const { t } = useI18n()
  const [range, setRange] = useState<Range>("1mo")
  const [nonce, setNonce] = useState(0)

  const dataset = range === "custom" ? DATASETS["1mo"] : DATASETS[range]
  const maxCalls = Math.max(...dataset.map((point) => point.calls)) || 1
  // Muted full-height track for buckets without calls, like the reference design.
  const chartData = dataset.map((point) => ({
    ...point,
    stub: point.calls > 0 ? 0 : maxCalls,
  }))

  const callsConfig = {
    calls: { label: t("homeLlmCalls"), color: "var(--chart-2)" },
  } satisfies ChartConfig
  const tokensConfig = {
    tokens: { label: t("homeTokens"), color: "var(--chart-1)" },
  } satisfies ChartConfig
  const hitConfig = {
    hit: { label: t("homeCacheHitRate"), color: "var(--chart-2)" },
  } satisfies ChartConfig

  return (
    <ScrollPane className="flex-1" innerClassName="flex flex-col gap-3 pb-2">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-xs text-muted-foreground">{t("homeMockHint")}</span>
        <div className="flex-1" />
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          value={range}
          onValueChange={(next) => {
            if (next) setRange(next as Range)
          }}
        >
          {RANGE_ITEMS.map((item) => (
            <ToggleGroupItem key={item.value} value={item.value}>
              {t(item.labelKey)}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t("refresh")}
          title={t("refresh")}
          onClick={() => setNonce((count) => count + 1)}
        >
          <RefreshCwIcon />
        </Button>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t("homeCallsTrendTitle")}</CardTitle>
        </CardHeader>
        <CardContent>
          <ChartContainer key={`calls-${nonce}`} config={callsConfig} className="h-56 w-full">
            <BarChart data={chartData}>
              <CartesianGrid vertical={false} />
              <XAxis dataKey="label" tickLine={false} axisLine={false} tickMargin={10} minTickGap={24} />
              <YAxis hide />
              <ChartTooltip cursor={false} content={<CallsTooltip />} />
              <Bar dataKey="stub" stackId="trend" fill="var(--muted)" radius={4} />
              <Bar dataKey="calls" stackId="trend" fill="var(--color-calls)" radius={4} />
            </BarChart>
          </ChartContainer>
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <StatCard label={t("homeCacheHitRate")} hint={t("hintCacheHitRate")} footnote="">
          <CardContent className="w-fit px-4">
            <div className="relative">
              <ChartContainer config={hitConfig} className="size-24">
                <RadialBarChart
                  data={[{ key: "hit", value: MOCK.hitRate }]}
                  startAngle={90}
                  endAngle={-270}
                  innerRadius={31}
                  outerRadius={40}
                >
                  <PolarAngleAxis type="number" domain={[0, 100]} tick={false} tickLine={false} axisLine={false} />
                  <RadialBar
                    dataKey="value"
                    background={{ fill: "var(--muted)" }}
                    cornerRadius={999}
                    fill="var(--color-hit)"
                  />
                </RadialBarChart>
              </ChartContainer>
              <div className="absolute inset-0 flex items-center justify-center text-sm font-semibold tabular-nums">
                {MOCK.hitRate.toFixed(2)}%
              </div>
            </div>
          </CardContent>
        </StatCard>

        <StatCard
          label={t("homeLlmCalls")}
          hint={t("hintLlmCalls")}
          footnote={t("homeCallsDetail", { ok: MOCK.callsOk, err: MOCK.callsErr })}
        >
          <ValueBody value={MOCK.callsTotal} />
        </StatCard>

        <StatCard
          label={t("homeTokens")}
          hint={t("hintTokens")}
          footnote={t("homePromptTokens", { count: MOCK.tokensPrompt })}
        >
          <ValueBody value={MOCK.tokensTotal} />
        </StatCard>

        <StatCard
          label={t("homeValueEstimate")}
          hint={t("hintValueEstimate")}
          footnote={t("homeCacheRw", { amount: MOCK.valueCacheRw })}
        >
          <ValueBody value={MOCK.valueTotal} />
        </StatCard>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t("homeTokensTrendTitle")}</CardTitle>
        </CardHeader>
        <CardContent>
          <ChartContainer key={`tokens-${nonce}`} config={tokensConfig} className="h-44 w-full">
            <LineChart data={dataset}>
              <CartesianGrid vertical={false} />
              <XAxis dataKey="label" tickLine={false} axisLine={false} tickMargin={10} minTickGap={24} />
              <YAxis hide domain={[0, "auto"]} />
              <ChartTooltip content={<ChartTooltipContent />} />
              <Line
                dataKey="tokens"
                type="monotone"
                stroke="var(--color-tokens)"
                strokeWidth={2}
                dot={false}
                activeDot={{ r: 4 }}
              />
            </LineChart>
          </ChartContainer>
        </CardContent>
      </Card>
    </ScrollPane>
  )
}

function StatCard({
  footnote,
  hint,
  label,
  children,
}: {
  footnote: string
  hint: string
  label: string
  children: ReactNode
}) {
  return (
    <Card className="gap-3 py-4">
      <CardHeader className="px-4">
        <CardTitle className="text-sm font-medium text-muted-foreground">{label}</CardTitle>
        <CardAction>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon-xs" aria-label={hint}>
                <InfoIcon />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{hint}</TooltipContent>
          </Tooltip>
        </CardAction>
      </CardHeader>
      {children}
      {footnote ? (
        <CardFooter className="px-4 text-xs text-muted-foreground">{footnote}</CardFooter>
      ) : null}
    </Card>
  )
}

function ValueBody({ value }: { value: string }) {
  return (
    <CardContent className="px-4">
      <div className="text-2xl font-semibold tabular-nums">{value}</div>
    </CardContent>
  )
}
