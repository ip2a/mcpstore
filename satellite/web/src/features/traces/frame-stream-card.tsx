import { useEffect, useState, type ReactNode } from "react"

import { useIsFetching, useQuery, useQueryClient } from "@tanstack/react-query"
import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react"

import { PanelCard } from "@/components/shared/panel-card"
import { WORKSPACE_TOOLBAR_CLASSNAME } from "@/components/shared/workspace-toolbar"
import { WorkspaceTabsList } from "@/components/shared/workspace-tabs-list"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
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
import { Tabs, TabsContent, TabsTrigger } from "@/components/ui/tabs"
import type { I18nKey } from "@/lib/i18n-core"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import { getFrameSessions, type FrameSessionSummary } from "@/lib/api"

type FrameFace = FrameSessionSummary["face"]
type FrameViewMode = "table" | "history" | "kv"

const PAGE_SIZES = [10, 20, 50] as const

/** MCP message stream for one face (gateway inbound or upstream outbound). */
export function FrameStreamCard(props: {
  face: FrameFace
  /** Fill parent height (Tools-style); table scrolls inside, page chrome stays put. */
  fill?: boolean
  /** Optional History tab content (Traces page only; Gateway omits this). */
  historySlot?: ReactNode
  /** Optional KV tab content (Traces page only; Gateway omits this). */
  kvSlot?: ReactNode
  initialView?: FrameViewMode
  /**
   * Parent owns tab chrome (e.g. Gateway workspace). Renders only this view
   * without PanelCard / internal tabs; refresh stays in a slim toolbar.
   */
  embedView?: "table"
  /** Open shared session detail page (replaces the old dialog). */
  onOpenSession?: (session: FrameSessionSummary) => void
}) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const fill = props.fill ?? false
  const embedded = props.embedView != null
  const [view, setView] = useState<FrameViewMode>(
    props.embedView ?? props.initialView ?? "table",
  )
  const [pageSize, setPageSize] = useState<(typeof PAGE_SIZES)[number]>(20)
  const [page, setPage] = useState(0)

  useEffect(() => {
    if (props.embedView) {
      setView(props.embedView)
      return
    }
    setView(props.initialView ?? "table")
  }, [props.embedView, props.initialView])

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
  const historyFetching = useIsFetching({ queryKey: queryKeys.gatewayHistory }) > 0
  const kvHealthFetching = useIsFetching({ queryKey: queryKeys.kvHealth })
  const kvInspectFetching = useIsFetching({ queryKey: queryKeys.kvInspect })
  const kvFetching = kvHealthFetching + kvInspectFetching > 0

  const items = sessions.data?.sessions ?? []
  const total = sessions.data?.total ?? 0
  const pageCount = Math.max(1, Math.ceil(total / pageSize))
  const emptyKey: I18nKey =
    props.face === "gateway" ? "gatewayFramesEmptyGateway" : "gatewayFramesEmptyUpstream"

  function refresh() {
    if (view === "history") {
      void queryClient.invalidateQueries({ queryKey: queryKeys.gatewayHistory })
      return
    }
    if (view === "kv") {
      void queryClient.invalidateQueries({ queryKey: queryKeys.kvHealth })
      void queryClient.invalidateQueries({ queryKey: queryKeys.kvInspect })
      return
    }
    void queryClient.invalidateQueries({
      queryKey: queryKeys.gatewayFrameSessions,
    })
  }

  const refreshSpinning =
    view === "history" ? historyFetching : view === "kv" ? kvFetching : sessions.isFetching

  function changePageSize(value: string) {
    setPageSize(Number(value) as (typeof PAGE_SIZES)[number])
    setPage(0)
  }

  const from = total === 0 ? 0 : offset + 1
  const to = Math.min(offset + items.length, total)

  const pager =
    total === 0 && !sessions.isLoading ? null : (
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
      <p className={cn("text-sm text-muted-foreground", fill && "px-4 py-3")}>{t(emptyKey)}</p>
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
                  className={props.onOpenSession ? "cursor-pointer" : undefined}
                  onClick={() => props.onOpenSession?.(session)}
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

  const refreshButton = (
    <Button disabled={refreshSpinning} size="sm" variant="outline" onClick={refresh}>
      <RefreshCwSpinIcon data-icon="inline-start" spinning={refreshSpinning} />
      {t("refresh")}
    </Button>
  )

  const body =
    view === "history" && props.historySlot ? (
      props.historySlot
    ) : view === "kv" && props.kvSlot ? (
      props.kvSlot
    ) : (
      tableBody
    )

  return (
    <>
      {embedded ? (
        <div
          className={cn(
            "flex flex-col",
            fill ? "min-h-0 flex-1 gap-0 overflow-hidden" : "gap-2",
          )}
        >
          <div
            className={cn(
              "flex justify-end",
              fill ? "shrink-0 px-4 py-2" : undefined,
            )}
          >
            {refreshButton}
          </div>
          <div className={cn(fill && "flex min-h-0 flex-1 flex-col overflow-hidden")}>
            {body}
          </div>
        </div>
      ) : (
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
            className={cn("flex flex-col overflow-hidden", fill ? "min-h-0 flex-1" : undefined)}
          >
            <div
              className={cn(
                fill
                  ? WORKSPACE_TOOLBAR_CLASSNAME
                  : "flex items-center justify-between border-b pb-2",
              )}
            >
              <WorkspaceTabsList value={view}>
                <TabsTrigger value="table" className="flex-none">
                  {t("gatewayFramesTabTable")}
                </TabsTrigger>
                {props.historySlot ? (
                  <TabsTrigger value="history" className="flex-none">
                    {t("navHistory")}
                  </TabsTrigger>
                ) : null}
                {props.kvSlot ? (
                  <TabsTrigger value="kv" className="flex-none">
                    {t("kv")}
                  </TabsTrigger>
                ) : null}
              </WorkspaceTabsList>
              <div className="ml-auto flex shrink-0 items-center gap-2">{refreshButton}</div>
            </div>

            <TabsContent value="table" className={tabContentClassName}>
              {tableBody}
            </TabsContent>
            {props.historySlot ? (
              <TabsContent value="history" className={tabContentClassName}>
                {props.historySlot}
              </TabsContent>
            ) : null}
            {props.kvSlot ? (
              <TabsContent value="kv" className={tabContentClassName}>
                {props.kvSlot}
              </TabsContent>
            ) : null}
          </Tabs>
        </PanelCard>
      )}
    </>
  )
}
