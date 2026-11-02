import { useEffect, useState } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import {
  ArrowDownIcon,
  ArrowUpIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  RefreshCwIcon,
} from "lucide-react"

import { PanelCard } from "@/components/shared/panel-card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import type { I18nKey } from "@/lib/i18n-core"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import {
  getFrameHistory,
  getFrameSessions,
  type FrameRecord,
  type FrameSessionSummary,
} from "@/lib/api"

type FrameFace = FrameSessionSummary["face"]
type FrameViewMode = "table" | "graph"

const PAGE_SIZES = [10, 20, 50] as const

/** MCP message stream for one face (gateway inbound or upstream outbound). */
export function FrameStreamCard(props: {
  face: FrameFace
  /** Fill parent height (Tools-style); table scrolls inside, page chrome stays put. */
  fill?: boolean
}) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const fill = props.fill ?? false
  const [view, setView] = useState<FrameViewMode>("table")
  const [pageSize, setPageSize] = useState<(typeof PAGE_SIZES)[number]>(20)
  const [page, setPage] = useState(0)
  const [active, setActive] = useState<FrameSessionSummary | null>(null)

  const offset = page * pageSize
  const sessions = useQuery({
    queryKey: queryKeys.gatewayFrameSessionsPage(props.face, pageSize, offset),
    queryFn: () =>
      getFrameSessions({
        face: props.face,
        hours: 24,
        limit: pageSize,
        offset,
      }),
  })

  const items = sessions.data?.sessions ?? []
  const total = sessions.data?.total ?? 0
  const pageCount = Math.max(1, Math.ceil(total / pageSize))
  const emptyKey: I18nKey =
    props.face === "gateway" ? "gatewayFramesEmptyGateway" : "gatewayFramesEmptyUpstream"

  function refresh() {
    void queryClient.invalidateQueries({
      queryKey: queryKeys.gatewayFrameSessions,
    })
    if (active) {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.gatewayFrameHistory(active.session_id),
      })
    }
  }

  function changePageSize(value: string) {
    setPageSize(Number(value) as (typeof PAGE_SIZES)[number])
    setPage(0)
  }

  const from = total === 0 ? 0 : offset + 1
  const to = Math.min(offset + items.length, total)

  const pager = total === 0 && !sessions.isLoading ? null : (
    <div
      className={cn(
        "flex flex-wrap items-center justify-between gap-2",
        fill ? "shrink-0 border-t px-4 py-2" : "border-t pt-2",
      )}
    >
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <span>{t("gatewayFramesPageSize")}</span>
        <Select value={String(pageSize)} onValueChange={changePageSize}>
          <SelectTrigger size="sm" className="h-8 w-[72px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {PAGE_SIZES.map((size) => (
              <SelectItem key={size} value={String(size)}>
                {size}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <span>{t("gatewayFramesShowing", { from, to, total })}</span>
      </div>
      <div className="flex items-center gap-1">
        <Button
          disabled={page <= 0}
          size="sm"
          variant="outline"
          onClick={() => setPage((prev) => Math.max(0, prev - 1))}
        >
          <ChevronLeftIcon className="size-3.5" />
          {t("gatewayFramesPrev")}
        </Button>
        <Button
          disabled={page + 1 >= pageCount}
          size="sm"
          variant="outline"
          onClick={() => setPage((prev) => prev + 1)}
        >
          {t("gatewayFramesNext")}
          <ChevronRightIcon className="size-3.5" />
        </Button>
      </div>
    </div>
  )

  const tabContentClassName = fill
    ? "mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
    : "mt-0 flex flex-col gap-2"

  const tableBody =
    total === 0 && !sessions.isLoading ? (
      <p className={cn("text-sm text-muted-foreground", fill && "px-4 py-3")}>
        {t(emptyKey)}
      </p>
    ) : (
      <>
        <div
          className={cn(
            fill &&
              "min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-4 [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden",
          )}
        >
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("gatewayFramesColPeer")}</TableHead>
                <TableHead>{t("gatewayFramesColInstance")}</TableHead>
                <TableHead>{t("gatewayFramesColLastSeen")}</TableHead>
                <TableHead className="text-right">{t("gatewayFramesColCount")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {items.map((session) => (
                <TableRow
                  key={session.session_id}
                  className="cursor-pointer"
                  onClick={() => setActive(session)}
                >
                  <TableCell className="max-w-[220px] truncate font-mono text-xs">
                    {session.peer ?? session.caller ?? session.session_id.slice(0, 12)}
                  </TableCell>
                  <TableCell className="max-w-[280px] truncate font-mono text-xs text-muted-foreground">
                    {session.instance_id ?? session.caller ?? "—"}
                  </TableCell>
                  <TableCell className="tabular-nums text-xs text-muted-foreground">
                    {new Date(session.last_ts_unix_ms).toLocaleString()}
                  </TableCell>
                  <TableCell className="text-right">
                    <Badge variant="secondary">{session.frame_count}</Badge>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
        {pager}
      </>
    )

  return (
    <>
      <PanelCard
        className={cn(
          fill ? "h-full min-h-0 flex-1 gap-0 p-0" : "gap-2",
          !fill && "flex flex-col",
        )}
        variant={fill ? "stack" : "plain"}
      >
        <Tabs
          value={view}
          onValueChange={(value) => setView(value as FrameViewMode)}
          className={cn(
            "flex flex-col overflow-hidden",
            fill ? "min-h-0 flex-1" : undefined,
          )}
        >
          <div
            className={cn(
              fill
                ? "flex w-full min-w-0 shrink-0 items-center gap-3 border-b px-4 py-2"
                : "flex items-center justify-between border-b pb-2",
            )}
          >
            <TabsList variant="line" className="h-9 w-fit shrink-0 justify-start">
              <TabsTrigger value="table" className="flex-none">
                {t("gatewayFramesTabTable")}
              </TabsTrigger>
              <TabsTrigger value="graph" className="flex-none">
                {t("gatewayFramesTabGraph")}
              </TabsTrigger>
            </TabsList>
            <div className="ml-auto flex shrink-0 items-center gap-2">
              <Button
                disabled={sessions.isFetching}
                size="sm"
                variant="outline"
                onClick={refresh}
              >
                <RefreshCwIcon data-icon="inline-start" />
                {t("refresh")}
              </Button>
            </div>
          </div>

          <TabsContent value="table" className={tabContentClassName}>
            {tableBody}
          </TabsContent>
          <TabsContent value="graph" className={tabContentClassName}>
            <div
              className={cn(
                "flex flex-1 items-center justify-center text-sm text-muted-foreground",
                fill ? "min-h-0 px-4 py-6" : "py-6",
              )}
            >
              {t("gatewayFramesGraphPlaceholder")}
            </div>
          </TabsContent>
        </Tabs>
      </PanelCard>

      <SessionTimelineDialog
        onOpenChange={(open) => {
          if (!open) setActive(null)
        }}
        session={active}
      />
    </>
  )
}

function SessionTimelineDialog({
  session,
  onOpenChange,
}: {
  session: FrameSessionSummary | null
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()
  const [openFrame, setOpenFrame] = useState<number | null>(null)
  useEffect(() => {
    setOpenFrame(null)
  }, [session?.session_id])
  const timeline = useQuery({
    queryKey: queryKeys.gatewayFrameHistory(session?.session_id ?? ""),
    enabled: session !== null,
    queryFn: () => getFrameHistory(session!.session_id),
  })

  return (
    <Dialog open={session !== null} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[min(85vh,720px)] flex-col gap-3 overflow-hidden sm:max-w-2xl">
        <DialogHeader className="shrink-0">
          <DialogTitle className="truncate font-mono text-base">
            {t("gatewayFramesDialogTitle", {
              id: session?.peer ?? session?.session_id.slice(0, 12) ?? "",
            })}
          </DialogTitle>
        </DialogHeader>
        <div className="min-h-0 flex-1 overflow-y-auto">
          {!timeline.data || timeline.data.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("gatewayFramesNoFrames")}</p>
          ) : (
            <div className="flex flex-col gap-1">
              {timeline.data.map((frame) => (
                <FrameRow
                  frame={frame}
                  key={frame.id}
                  open={openFrame === frame.id}
                  onToggle={() =>
                    setOpenFrame((prev) => (prev === frame.id ? null : frame.id))
                  }
                />
              ))}
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
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
