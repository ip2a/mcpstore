import { useState } from "react"
import { useQuery } from "@tanstack/react-query"

import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Badge } from "@/components/ui/badge"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import { getRequestHistory, type RequestRecord } from "@/lib/api"

/** Insights → History: daemon telemetry request log (tools + management). */
export function HistoryView() {
  return (
    <ScrollPane className="flex-1" innerClassName="flex flex-col gap-3 pb-2">
      <RequestHistoryCard />
    </ScrollPane>
  )
}

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
    // non-JSON (truncated residue) shown as-is
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
