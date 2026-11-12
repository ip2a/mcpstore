import { useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { ArrowDownIcon, ArrowUpIcon } from "lucide-react"

import { PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { WorkspaceBreadcrumb } from "@/components/shared/workspace-page-header"
import {
  WORKSPACE_LIST_INSET_CLASSNAME,
  WORKSPACE_PANEL_CLASSNAME,
} from "@/components/shared/workspace-toolbar"
import { Badge } from "@/components/ui/badge"
import { getFrameHistory, type FrameRecord } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"

/** Shared session detail: JSON-RPC frame timeline for one connection. */
export function TraceSessionDetailView(props: {
  sessionId: string
  face: "gateway" | "upstream"
  onBack: () => void
}) {
  const { t } = useI18n()
  const [openFrame, setOpenFrame] = useState<number | null>(null)
  const timeline = useQuery({
    queryKey: queryKeys.gatewayFrameHistory(props.sessionId),
    queryFn: () => getFrameHistory(props.sessionId),
  })

  const peer =
    timeline.data?.find((frame) => frame.peer)?.peer ??
    timeline.data?.find((frame) => frame.caller)?.caller ??
    null
  const titleId = peer ?? props.sessionId.slice(0, 12)
  const parentLabel = props.face === "gateway" ? t("gateway") : t("navTraces")

  return (
    <>
      <WorkspaceBreadcrumb
        ariaLabel={t("navTraces")}
        crumbs={[
          { label: parentLabel, onClick: props.onBack },
          { label: t("gatewayFramesDialogTitle", { id: titleId }) },
        ]}
      />
      <PanelCard className={WORKSPACE_PANEL_CLASSNAME}>
        {timeline.isLoading ? (
          <PageSkeleton />
        ) : !timeline.data || timeline.data.length === 0 ? (
          <p className={cn(WORKSPACE_LIST_INSET_CLASSNAME, "text-sm text-muted-foreground")}>
            {t("gatewayFramesNoFrames")}
          </p>
        ) : (
          <ScrollPane
            className="min-h-0 flex-1"
            innerClassName={cn(WORKSPACE_LIST_INSET_CLASSNAME, "flex flex-col gap-1")}
          >
            {timeline.data.map((frame) => (
              <FrameRow
                frame={frame}
                key={frame.id}
                open={openFrame === frame.id}
                onToggle={() => setOpenFrame((prev) => (prev === frame.id ? null : frame.id))}
              />
            ))}
          </ScrollPane>
        )}
      </PanelCard>
    </>
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
        <span className="shrink-0 text-xs text-muted-foreground">{frame.size_bytes}B</span>
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
    // Not JSON (truncated leftover) — show as-is
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
