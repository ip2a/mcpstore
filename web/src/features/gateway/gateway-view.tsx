import { useEffect, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { CableIcon, WaypointsIcon } from "lucide-react"
import { toast } from "sonner"

import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  useClientsQuery,
  useGatewayStatusQuery,
  useWiredClientsQuery,
} from "@/features/clients/queries"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import {
  getGatewayRequests,
  getRequestHistory,
  startMcpHub,
  stopMcpHub,
  type GatewayRequestRecord,
  type RequestRecord,
  type WiredEntry,
} from "@/lib/api"

export function GatewayView() {
  const { t } = useI18n()
  const status = useGatewayStatusQuery()
  const clients = useClientsQuery()
  const wired = useWiredClientsQuery()
  const records = useGatewayRequestFlow()
  const [busy, setBusy] = useState(false)

  const wiredLabels = (clients.data ?? [])
    .filter((client) => client.wired)
    .map((client) => client.label)
  const labelById = new Map((clients.data ?? []).map((client) => [client.id, client.label]))

  async function handleStart() {
    setBusy(true)
    try {
      await startMcpHub({ transport: "streamable-http", scope: "store" })
      await status.refetch()
      toast.success(t("gatewayStarted"))
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  async function handleStop() {
    setBusy(true)
    try {
      await stopMcpHub()
      await status.refetch()
      toast.success(t("gatewayStopped"))
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  return (
    <ScrollPane className="flex-1" innerClassName="flex flex-col gap-3 pb-2">
      <PanelCard variant="plain" className="gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b pb-3">
          <div className="flex items-center gap-2">
            <strong className="font-mono text-sm font-medium">{t("gateway")}</strong>
            {status.data?.running ? (
              <Badge variant="default">{t("homeRunning")}</Badge>
            ) : (
              <Badge variant="destructive">{t("gatewayStopped")}</Badge>
            )}
          </div>
          <div className="flex gap-2">
            {status.data?.running ? (
              <Button disabled={busy} size="sm" variant="outline" onClick={handleStop}>
                {t("gatewayStop")}
              </Button>
            ) : (
              <Button disabled={busy} size="sm" onClick={handleStart}>
                {t("gatewayStart")}
              </Button>
            )}
          </div>
        </div>
        <div className="flex flex-col gap-1 font-mono text-xs text-muted-foreground">
          <span className="truncate">
            {t("gatewayUrl")}: {status.data?.url ?? "—"}
          </span>
          <span className="truncate">
            {t("clientsWired")}: {wiredLabels.length > 0 ? wiredLabels.join(", ") : "—"}
          </span>
        </div>
      </PanelCard>

      <WiredClientsCard labelById={labelById} wired={wired.data ?? []} />

      <PanelCard variant="plain" className="gap-2">
        <div className="border-b pb-2 text-sm font-medium">{t("gatewayRequestsTitle")}</div>
        {records.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t("gatewayRequestsEmpty")}</p>
        ) : (
          [...records].reverse().map((record) => <FlowRow key={record.seq} record={record} />)
        )}
      </PanelCard>

      <RequestHistoryCard />
    </ScrollPane>
  )
}

/** 长轮询循环：等新记录或 25s 超时后继续；后端重启（seq 回退）时清空重追。 */
function useGatewayRequestFlow() {
  const [records, setRecords] = useState<GatewayRequestRecord[]>([])
  useEffect(() => {
    let cancelled = false
    let after = 0
    void (async () => {
      while (!cancelled) {
        try {
          const page = await getGatewayRequests(after, 25_000)
          if (cancelled) return
          if (page.latest_seq < after) {
            after = 0
            setRecords([])
          }
          if (page.requests.length > 0) {
            after = page.latest_seq
            setRecords((prev) => [...prev, ...page.requests].slice(-200))
          } else if (!page.running) {
            // 网关未运行时后端立即返回（不走 25s 长等），必须节流否则全速空转
            await new Promise((resolve) => setTimeout(resolve, 3_000))
          }
        } catch {
          await new Promise((resolve) => setTimeout(resolve, 3_000))
        }
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])
  return records
}

function FlowRow({ record }: { record: GatewayRequestRecord }) {
  const { t } = useI18n()
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-md border border-dashed px-3 py-2">
      <Badge variant="outline" className="shrink-0 font-mono">
        {record.caller ?? t("gatewayCallDirect")}
      </Badge>
      <WaypointsIcon className="size-3.5 shrink-0 text-muted-foreground" />
      <span className="min-w-0 flex-1 truncate font-mono text-sm">
        {record.service_name}.{record.tool}
      </span>
      <span className="flex shrink-0 items-center gap-2">
        <Badge variant={record.success ? "secondary" : "destructive"}>
          {record.success ? "OK" : "ERR"}
        </Badge>
        <span className="text-xs text-muted-foreground">{record.duration_ms}ms</span>
      </span>
    </div>
  )
}

/** 已接入客户端：每个挂载点的作用域 + 暴露配置（/gateway/wired 返回生效值）。 */
function WiredClientsCard({
  labelById,
  wired,
}: {
  labelById: Map<string, string>
  wired: WiredEntry[]
}) {
  const { t } = useI18n()
  return (
    <PanelCard variant="plain" className="gap-2">
      <div className="border-b pb-2 text-sm font-medium">{t("gatewayWiredTitle")}</div>
      {wired.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t("gatewayWiredEmpty")}</p>
      ) : (
        wired.map((entry) => (
          <WiredClientRow key={entry.client_id} entry={entry} labelById={labelById} />
        ))
      )}
    </PanelCard>
  )
}

function WiredClientRow({
  entry,
  labelById,
}: {
  entry: WiredEntry
  labelById: Map<string, string>
}) {
  const { t } = useI18n()
  const exposeOn = Object.entries(entry.expose)
    .filter(([, on]) => on)
    .map(([key]) => key)
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-md border border-dashed px-3 py-2">
      <CableIcon className="size-3.5 shrink-0 text-muted-foreground" />
      <span className="min-w-0 truncate text-sm font-medium">
        {labelById.get(entry.client_id) ?? entry.client_id}
      </span>
      <Badge variant="secondary" className="shrink-0 font-mono text-xs">
        {entry.scope.type === "agent" ? `Agent:${entry.scope.agent_id}` : t("gatewayScopeStore")}
      </Badge>
      {exposeOn.length > 0 ? (
        <span className="min-w-0 truncate font-mono text-xs text-muted-foreground">
          {exposeOn.join(", ")}
        </span>
      ) : (
        <span className="min-w-0 truncate text-xs text-muted-foreground">
          {t("gatewayExposeNone")}
        </span>
      )}
      <span className="ml-auto min-w-0 truncate font-mono text-xs text-muted-foreground">
        {entry.url}
      </span>
    </div>
  )
}

/** 历史请求：daemon 遥测持久层最近 50 条（工具调用 + 管理操作），点击行展开完整参数/返回。 */
function RequestHistoryCard() {
  const { t } = useI18n()
  const history = useQuery({
    queryKey: queryKeys.gatewayHistory,
    queryFn: () => getRequestHistory({ limit: 50 }),
    refetchInterval: 15_000,
  })
  const records = history.data?.requests ?? []
  return (
    <PanelCard variant="plain" className="gap-2">
      <div className="border-b pb-2 text-sm font-medium">{t("gatewayHistoryTitle")}</div>
      {records.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t("gatewayHistoryEmpty")}</p>
      ) : (
        <div className="flex flex-col gap-1">
          {records.map((record) => (
            <HistoryRow key={record.id} record={record} />
          ))}
        </div>
      )}
    </PanelCard>
  )
}

function HistoryRow({ record }: { record: RequestRecord }) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  return (
    <div className="rounded-md border border-dashed">
      <button
        type="button"
        className="flex w-full flex-wrap items-center gap-2 px-3 py-2 text-left"
        onClick={() => setOpen((prev) => !prev)}
      >
        <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
          {new Date(record.started_at_unix_ms).toLocaleTimeString()}
        </span>
        <Badge variant="outline" className="shrink-0 font-mono">
          {record.caller ?? t("gatewayCallDirect")}
        </Badge>
        <span className="min-w-0 flex-1 truncate font-mono text-sm">
          {record.kind === "tool"
            ? `${record.service_name ?? "—"}.${record.tool ?? "—"}`
            : record.tool ?? "—"}
        </span>
        <Badge variant="secondary" className="shrink-0">
          {record.kind === "tool" ? t("gatewayKindTool") : t("gatewayKindManagement")}
        </Badge>
        <Badge variant={record.success ? "secondary" : "destructive"} className="shrink-0">
          {record.success ? "OK" : "ERR"}
        </Badge>
        <span className="shrink-0 text-xs text-muted-foreground">{record.duration_ms}ms</span>
      </button>
      {open ? (
        <div className="flex flex-col gap-2 border-t px-3 py-2 text-xs">
          {record.error_summary ? (
            <div>
              <span className="font-medium text-destructive">
                {record.error_code ?? "ERR"}
              </span>
              <pre className="mt-1 max-h-32 overflow-auto whitespace-pre-wrap break-all rounded bg-muted/50 p-2">
                {record.error_summary}
              </pre>
            </div>
          ) : null}
          <PayloadBlock
            title={t("gatewayDetailArgs")}
            payload={record.args_json}
            truncated={record.args_truncated}
          />
          <PayloadBlock
            title={t("gatewayDetailResult")}
            payload={record.result_json}
            truncated={record.result_truncated}
          />
        </div>
      ) : null}
    </div>
  )
}

function PayloadBlock({
  title,
  payload,
  truncated,
}: {
  title: string
  payload: string | null
  truncated: boolean
}) {
  const { t } = useI18n()
  if (!payload) return null
  let pretty = payload
  try {
    pretty = JSON.stringify(JSON.parse(payload), null, 2)
  } catch {
    // 非 JSON（截断残留）原样展示
  }
  return (
    <div className="min-w-0">
      <div className="flex items-center gap-2">
        <span className="font-medium">{title}</span>
        {truncated ? (
          <Badge variant="outline" className="text-[10px]">
            {t("gatewayTruncated")}
          </Badge>
        ) : null}
      </div>
      <pre className="mt-1 max-h-64 overflow-auto whitespace-pre-wrap break-all rounded bg-muted/50 p-2">
        {pretty}
      </pre>
    </div>
  )
}
