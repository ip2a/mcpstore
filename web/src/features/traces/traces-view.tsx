import { useState } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import { ArrowDownIcon, ArrowUpIcon, RefreshCwIcon } from "lucide-react"

import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import type { I18nKey } from "@/lib/i18n-core"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import {
  getFrameHistory,
  getFrameSessions,
  getRequestHistory,
  type FrameRecord,
  type FrameSessionSummary,
  type RequestRecord,
} from "@/lib/api"

/** Insights → Traces: Request history + MCP message stream (moved from Gateway). */
export function TracesView() {
  return (
    <ScrollPane className="flex-1" innerClassName="flex flex-col gap-3 pb-2">
      <RequestHistoryCard />
      <FrameStreamCard />
    </ScrollPane>
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

/** MCP 消息流：网关与对外连接的 JSON-RPC 帧会话（~/.mcpstore/telemetry.db frames 表）。 */
function FrameStreamCard() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const [active, setActive] = useState<string | null>(null)
  const [openFrame, setOpenFrame] = useState<number | null>(null)
  const sessions = useQuery({
    queryKey: queryKeys.gatewayFrameSessions,
    queryFn: () => getFrameSessions({ hours: 24 }),
  })
  const timeline = useQuery({
    queryKey: queryKeys.gatewayFrameHistory(active ?? ""),
    enabled: active !== null,
    queryFn: () => getFrameHistory(active as string),
  })

  const groups: { face: FrameSessionSummary["face"]; key: I18nKey }[] = [
    { face: "gateway", key: "gatewayFramesFaceGateway" },
    { face: "upstream", key: "gatewayFramesFaceUpstream" },
  ]
  const all = sessions.data ?? []

  function refresh() {
    void queryClient.invalidateQueries({ queryKey: queryKeys.gatewayFrameSessions })
    if (active) {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.gatewayFrameHistory(active),
      })
    }
  }

  return (
    <PanelCard variant="plain" className="gap-2">
      <div className="flex items-center justify-between border-b pb-2">
        <div className="text-sm font-medium">{t("gatewayFramesTitle")}</div>
        <Button
          disabled={sessions.isFetching}
          size="sm"
          variant="ghost"
          onClick={refresh}
        >
          <RefreshCwIcon className="size-3.5" />
        </Button>
      </div>
      {all.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t("gatewayFramesEmpty")}</p>
      ) : (
        <div className="flex flex-col gap-2">
          {groups.map(({ face, key }) => {
            const items = all.filter((session) => session.face === face)
            if (items.length === 0) return null
            return (
              <div key={face} className="flex flex-col gap-1">
                <div className="text-xs font-medium text-muted-foreground">
                  {t(key)}
                </div>
                {items.map((session) => (
                  <SessionRow
                    active={session.session_id === active}
                    key={session.session_id}
                    onSelect={() => {
                      setActive(session.session_id)
                      setOpenFrame(null)
                    }}
                    session={session}
                  />
                ))}
              </div>
            )
          })}
          {active && timeline.data ? (
            <div className="flex flex-col gap-1 border-t pt-2">
              {timeline.data.length === 0 ? (
                <p className="text-sm text-muted-foreground">
                  {t("gatewayFramesNoFrames")}
                </p>
              ) : (
                timeline.data.map((frame) => (
                  <FrameRow
                    frame={frame}
                    key={frame.id}
                    open={openFrame === frame.id}
                    onToggle={() =>
                      setOpenFrame((prev) => (prev === frame.id ? null : frame.id))
                    }
                  />
                ))
              )}
            </div>
          ) : null}
        </div>
      )}
    </PanelCard>
  )
}

function SessionRow({
  session,
  active,
  onSelect,
}: {
  session: FrameSessionSummary
  active: boolean
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      className={`flex w-full flex-wrap items-center gap-2 rounded-md border px-3 py-2 text-left ${
        active ? "border-solid bg-muted/50" : "border-dashed"
      }`}
      onClick={onSelect}
    >
      <Badge variant="outline" className="shrink-0 font-mono">
        {session.peer ?? session.caller ?? session.session_id.slice(0, 12)}
      </Badge>
      <span className="min-w-0 flex-1 truncate font-mono text-xs text-muted-foreground">
        {session.instance_id ?? session.caller ?? "—"}
      </span>
      <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
        {new Date(session.last_ts_unix_ms).toLocaleTimeString()}
      </span>
      <Badge variant="secondary" className="shrink-0">
        {session.frame_count}
      </Badge>
    </button>
  )
}

function FrameRow({
  frame,
  open,
  onToggle,
}: {
  frame: FrameRecord
  open: boolean
  onToggle: () => void
}) {
  const Arrow = frame.direction === "up" ? ArrowUpIcon : ArrowDownIcon
  return (
    <div className="rounded-md border border-dashed">
      <button
        type="button"
        className="flex w-full items-center gap-2 px-3 py-2 text-left"
        onClick={onToggle}
      >
        <Arrow
          className={`size-3.5 shrink-0 ${
            frame.direction === "up" ? "text-blue-500" : "text-emerald-500"
          }`}
        />
        <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
          {new Date(frame.ts_unix_ms).toLocaleTimeString()}
        </span>
        <span className="min-w-0 flex-1 truncate font-mono text-sm">
          {frame.method ?? `#${frame.jsonrpc_id ?? "—"}`}
        </span>
        <Badge
          variant={
            frame.kind === "error"
              ? "destructive"
              : frame.kind === "notification"
                ? "outline"
                : "secondary"
          }
          className="shrink-0"
        >
          {frame.kind}
        </Badge>
        <span className="shrink-0 text-xs text-muted-foreground">
          {frame.size_bytes}B
        </span>
      </button>
      {open ? (
        <div className="border-t px-3 py-2 text-xs">
          <PayloadBlock
            payload={frame.payload}
            title={`${frame.face} / ${frame.kind}`}
            truncated={frame.truncated}
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
