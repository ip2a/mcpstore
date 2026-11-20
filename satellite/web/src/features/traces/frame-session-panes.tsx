import { ArrowDownIcon, ArrowUpIcon } from "lucide-react"

import { ScrollPane } from "@/components/shared/scroll-pane"
import { WORKSPACE_LIST_INSET_CLASSNAME } from "@/components/shared/workspace-toolbar"
import { Badge } from "@/components/ui/badge"
import type { FrameRecord } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

/** Compact timeline row: time · method · kind. */
export function FrameTimelineRow({
  frame,
  selected,
  onSelect,
}: {
  frame: FrameRecord
  selected: boolean
  onSelect: () => void
}) {
  const Arrow = frame.direction === "up" ? ArrowUpIcon : ArrowDownIcon
  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        "flex w-full items-center gap-2 rounded-md border px-3 py-2 text-left transition-colors",
        selected
          ? "border-border bg-secondary"
          : "border-transparent hover:border-border/60 hover:bg-muted/40",
      )}
    >
      <Arrow
        className={cn(
          "size-3.5 shrink-0",
          frame.direction === "up" ? "text-blue-500" : "text-emerald-500",
        )}
      />
      <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
        {new Date(frame.ts_unix_ms).toLocaleTimeString(undefined, {
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
          hour12: false,
        })}
      </span>
      <span className="min-w-0 flex-1 truncate font-mono text-xs">
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
    </button>
  )
}

/** Selected-frame meta + pretty JSON payload. */
export function FrameDetailPane({
  frame,
  className,
  insetClassName,
}: {
  frame: FrameRecord
  className?: string
  insetClassName?: string
}) {
  const { t } = useI18n()
  const Arrow = frame.direction === "up" ? ArrowUpIcon : ArrowDownIcon
  const fullTime = new Date(frame.ts_unix_ms).toLocaleString()
  const meta = [
    frame.face,
    frame.direction,
    frame.transport,
    frame.peer,
    frame.caller,
    frame.jsonrpc_id != null ? `#${frame.jsonrpc_id}` : null,
    `${frame.size_bytes}B`,
  ]
    .filter(Boolean)
    .join(" · ")

  return (
    <ScrollPane
      className={cn("min-h-0 flex-1", className)}
      innerClassName={cn(insetClassName ?? WORKSPACE_LIST_INSET_CLASSNAME, "flex flex-col gap-3")}
    >
      <div className="flex flex-col gap-2 border-b pb-3">
        <div className="flex flex-wrap items-center gap-2">
          <Arrow
            className={cn(
              "size-4 shrink-0",
              frame.direction === "up" ? "text-blue-500" : "text-emerald-500",
            )}
          />
          <h2 className="min-w-0 truncate font-mono text-base font-semibold">
            {frame.method ?? `#${frame.jsonrpc_id ?? "—"}`}
          </h2>
          <Badge
            variant={
              frame.kind === "error"
                ? "destructive"
                : frame.kind === "notification"
                  ? "outline"
                  : "secondary"
            }
          >
            {frame.kind}
          </Badge>
          {frame.truncated ? (
            <Badge variant="outline" className="text-[10px]">
              {t("gatewayTruncated")}
            </Badge>
          ) : null}
        </div>
        <p className="text-xs tabular-nums text-muted-foreground" title={fullTime}>
          {fullTime}
        </p>
        <p className="truncate font-mono text-xs text-muted-foreground" title={meta}>
          {meta}
        </p>
      </div>
      <PayloadBlock payload={frame.payload} />
    </ScrollPane>
  )
}

function PayloadBlock({ payload }: { payload: string | null }) {
  if (!payload) return null
  let pretty = payload
  try {
    pretty = JSON.stringify(JSON.parse(payload), null, 2)
  } catch {
    // Not JSON (truncated leftover) — show as-is
  }
  return (
    <pre className="max-h-none min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-all rounded-md bg-muted/50 p-3 font-mono text-xs">
      {pretty}
    </pre>
  )
}
