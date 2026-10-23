import { type ComponentProps, type ReactNode } from "react"
import { InfoIcon } from "lucide-react"
import { PolarAngleAxis, RadialBar, RadialBarChart } from "recharts"

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
  type ChartConfig,
} from "@/components/ui/chart"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

// TODO(api): swap for stats API once the backend exposes usage metrics.
export const OVERVIEW_STAT_MOCK = {
  hitRate: 52.41,
  callsTotal: "2.1K",
  callsOk: "1.9K",
  callsErr: "145",
  tokensTotal: "282M",
  tokensPrompt: "280M",
  valueTotal: "$790.59",
  valueCacheRw: "$73.53",
}

export function OverviewStatCards({ className }: { className?: string }) {
  const { t } = useI18n()
  const hitConfig = {
    hit: { label: t("homeCacheHitRate"), color: "var(--chart-2)" },
  } satisfies ChartConfig

  return (
    <div className={cn("grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4", className)}>
      <StatCard label={t("homeCacheHitRate")} hint={t("hintCacheHitRate")} footnote="">
        <CardContent className="w-fit px-4">
          <div className="relative">
            <ChartContainer config={hitConfig} className="size-24">
              <RadialBarChart
                data={[{ key: "hit", value: OVERVIEW_STAT_MOCK.hitRate }]}
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
              {OVERVIEW_STAT_MOCK.hitRate.toFixed(2)}%
            </div>
          </div>
        </CardContent>
      </StatCard>

      <StatCard
        label={t("homeLlmCalls")}
        hint={t("hintLlmCalls")}
        footnote={t("homeCallsDetail", { ok: OVERVIEW_STAT_MOCK.callsOk, err: OVERVIEW_STAT_MOCK.callsErr })}
      >
        <ValueBody value={OVERVIEW_STAT_MOCK.callsTotal} />
      </StatCard>

      <StatCard
        label={t("homeTokens")}
        hint={t("hintTokens")}
        footnote={t("homePromptTokens", { count: OVERVIEW_STAT_MOCK.tokensPrompt })}
      >
        <ValueBody value={OVERVIEW_STAT_MOCK.tokensTotal} />
      </StatCard>

      <StatCard
        label={t("homeValueEstimate")}
        hint={t("hintValueEstimate")}
        footnote={t("homeCacheRw", { amount: OVERVIEW_STAT_MOCK.valueCacheRw })}
      >
        <ValueBody value={OVERVIEW_STAT_MOCK.valueTotal} />
      </StatCard>
    </div>
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
