import { BotIcon, HammerIcon, WaypointsIcon, type LucideIcon } from "lucide-react"

import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Badge } from "@/components/ui/badge"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

// TODO(api): mock data — swap for the gateway routing log and aggregated
// totals once the backend exposes them.
type GatewayRoute = {
  id: string
  caller: string
  callerDetail: string
  service: string
  tool: string
  code: number
  ok: boolean
  latencyMs: number
}

const HUB = { name: "mcpstore", addr: "127.0.0.1:1820" }

const MOCK_ROUTES: GatewayRoute[] = [
  {
    id: "github-create-issue",
    caller: "Claude Code",
    callerDetail: "claude-haiku-5-5",
    service: "github",
    tool: "create_issue",
    code: 200,
    ok: true,
    latencyMs: 84,
  },
  {
    id: "filesystem-read-file",
    caller: "Cursor",
    callerDetail: "composer-2.5",
    service: "filesystem",
    tool: "read_file",
    code: 200,
    ok: true,
    latencyMs: 12,
  },
  {
    id: "database-query",
    caller: "Claude Code",
    callerDetail: "claude-haiku-5-5",
    service: "database",
    tool: "query",
    code: 503,
    ok: false,
    latencyMs: 1204,
  },
]

const MOCK_TOTALS = { requests: "3,391", reroutes: "0", errors: "173", hitRate: "99%" }

export function GatewayView() {
  const { t } = useI18n()

  return (
    <ScrollPane className="flex-1" innerClassName="flex flex-col gap-3 pb-2">
      <PanelCard variant="plain" className="gap-4">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b pb-3">
          <strong className="font-mono text-sm font-medium">{t("homeRouteTitle")}</strong>
          <span className="text-xs text-muted-foreground">{t("homeMockHint")}</span>
        </div>
        <MetricGrid columns="four">
          <MetricTile variant="compact" label={t("homeRouteRequests")} value={MOCK_TOTALS.requests} />
          <MetricTile variant="compact" label={t("homeRouteReroutes")} value={MOCK_TOTALS.reroutes} />
          <MetricTile variant="compact" label={t("homeRouteErrors")} value={MOCK_TOTALS.errors} />
          <MetricTile variant="compact" label={t("homeCacheHitRate")} value={MOCK_TOTALS.hitRate} />
        </MetricGrid>
      </PanelCard>

      {MOCK_ROUTES.map((route) => (
        <RouteCard key={route.id} route={route} />
      ))}
    </ScrollPane>
  )
}

function RouteCard({ route }: { route: GatewayRoute }) {
  const { t } = useI18n()
  const target = `${route.service}@${route.tool}`

  return (
    <PanelCard variant="plain" className="gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <FlowNode detail={route.callerDetail} icon={BotIcon} name={route.caller} />
        <span aria-hidden className="h-px w-5 shrink-0 bg-border" />
        <FlowNode accent detail={HUB.addr} icon={WaypointsIcon} name={HUB.name} />
        <span aria-hidden className="h-px w-5 shrink-0 bg-border" />
        <div className="flex min-w-[16rem] flex-1 flex-wrap items-center justify-between gap-x-3 gap-y-1 rounded-md border border-dashed px-3 py-2">
          <span className="flex min-w-0 items-center gap-2">
            <HammerIcon className="size-4 shrink-0 text-muted-foreground" />
            <span className="truncate font-mono text-sm">{target}</span>
          </span>
          <span className="flex items-center gap-2">
            <Badge variant={route.ok ? "default" : "destructive"}>
              {route.code} · {route.ok ? t("homeRouteOk") : t("homeRouteFail")}
            </Badge>
            <span className="text-xs text-muted-foreground">
              {t("homeRouteReturned", { caller: route.caller })} · {route.latencyMs}ms
            </span>
          </span>
        </div>
      </div>

      <p className="text-sm text-muted-foreground">
        {t("homeRouteSummary", {
          caller: route.caller,
          target,
          ms: route.latencyMs,
          hit: MOCK_TOTALS.hitRate,
        })}
      </p>
    </PanelCard>
  )
}

function FlowNode({
  accent = false,
  detail,
  icon: Icon,
  name,
}: {
  accent?: boolean
  detail: string
  icon: LucideIcon
  name: string
}) {
  return (
    <div
      className={cn(
        "flex shrink-0 items-center gap-2 rounded-md border px-3 py-2",
        accent ? "border-primary/60 bg-primary/5" : "border-border/50",
      )}
    >
      <Icon className="size-4 shrink-0 text-muted-foreground" />
      <span className="flex flex-col leading-tight">
        <span className="text-sm font-medium">{name}</span>
        <span className="font-mono text-xs text-muted-foreground">{detail}</span>
      </span>
    </div>
  )
}
