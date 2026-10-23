import { useEffect, useState } from "react"
import { WaypointsIcon } from "lucide-react"
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
import { getGatewayRequests, startMcpHub, stopMcpHub, type GatewayRequestRecord } from "@/lib/api"

export function GatewayView() {
  const { t } = useI18n()
  const status = useGatewayStatusQuery()
  const clients = useClientsQuery()
  const records = useGatewayRequestFlow()
  const [busy, setBusy] = useState(false)

  const wiredLabels = (clients.data ?? [])
    .filter((client) => client.wired)
    .map((client) => client.label)

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

      <PanelCard variant="plain" className="gap-2">
        <div className="border-b pb-2 text-sm font-medium">{t("gatewayRequestsTitle")}</div>
        {records.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t("gatewayRequestsEmpty")}</p>
        ) : (
          [...records].reverse().map((record) => <FlowRow key={record.seq} record={record} />)
        )}
      </PanelCard>
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
