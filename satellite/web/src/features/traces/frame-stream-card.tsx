import { useEffect, useMemo, useState, type ReactNode } from "react"

import { useIsFetching, useQuery, useQueryClient } from "@tanstack/react-query"
import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react"

import { PanelCard } from "@/components/shared/panel-card"
import { UiverseSearchInput } from "@/components/shared/uiverse-search-input"
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
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import {
  FramesFilterDialog,
  type FrameSessionSortBy,
} from "@/features/traces/frames-filter-dialog"
import {
  TRACE_COL1_CELL_CLASSNAME,
  TRACE_COL1_CLASSNAME,
  TRACE_COL2_CELL_CLASSNAME,
  TRACE_COL2_CLASSNAME,
  TRACE_COL_TAGS_CLASSNAME,
  TRACE_TABLE_CLASSNAME,
} from "@/features/traces/trace-table-layout"
import type { I18nKey } from "@/lib/i18n-core"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import { getFrameSessions, type FrameSessionSummary } from "@/lib/api"

type FrameFace = FrameSessionSummary["face"]
type FrameViewMode = "table" | "history" | "kv"

const PAGE_SIZES = [10, 20, 50] as const

function formatByteSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B"
  if (bytes < 1024) return `${Math.round(bytes)} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0)} MB`
}

function sameLocalDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  )
}

function formatClock(date: Date): string {
  return date.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  })
}

function collectScopes(sessions: FrameSessionSummary[]): string[] {
  const seen = new Set<string>()
  for (const session of sessions) {
    if (session.scope) seen.add(session.scope)
  }
  return [...seen].sort((a, b) => a.localeCompare(b))
}

function filterAndSortSessions(
  sessions: FrameSessionSummary[],
  query: string,
  scopeFilter: string,
  sortBy: FrameSessionSortBy | null,
): FrameSessionSummary[] {
  const needle = query.trim().toLowerCase()
  let next = sessions.filter((session) => {
    if (scopeFilter !== "all" && session.scope !== scopeFilter) return false
    if (!needle) return true
    const haystack = [
      session.session_id,
      session.service_name,
      session.scope,
      session.peer,
      session.caller,
      session.instance_id,
    ]
      .filter(Boolean)
      .join(" ")
      .toLowerCase()
    return haystack.includes(needle)
  })
  if (sortBy) {
    next = [...next].sort((a, b) => {
      switch (sortBy) {
        case "time":
          return b.last_ts_unix_ms - a.last_ts_unix_ms
        case "frames":
          return b.frame_count - a.frame_count
        case "request":
          return b.request_bytes - a.request_bytes
        case "response":
          return b.response_bytes - a.response_bytes
        case "service":
          return (a.service_name ?? "").localeCompare(b.service_name ?? "")
      }
    })
  }
  return next
}

/** Clock times in cell; full date+time in tooltip. Cross-day end renders in red. */
function SessionActiveRange({
  firstMs,
  lastMs,
}: {
  firstMs: number
  lastMs: number
}) {
  const first = new Date(firstMs)
  const last = new Date(lastMs)
  const fullRange = `${first.toLocaleString()} – ${last.toLocaleString()}`
  const crossDay = !sameLocalDay(first, last)

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="cursor-default">
          {formatClock(first)} –{" "}
          {crossDay ? (
            <span className="text-destructive">{formatClock(last)}</span>
          ) : (
            formatClock(last)
          )}
        </span>
      </TooltipTrigger>
      <TooltipContent side="top" sideOffset={6}>
        {fullRange}
      </TooltipContent>
    </Tooltip>
  )
}

/** MCP message stream for one face (gateway inbound or upstream outbound). */
export function FrameStreamCard(props: {
  face: FrameFace
  /** Fill parent height (Tools-style); table scrolls inside, page chrome stays put. */
  fill?: boolean
  /** Optional History tab content (Traces page only; Gateway omits this). */
  historySlot?: ReactNode
  /** Optional KV tab content (Traces page only; Gateway omits this). */
  kvSlot?: (search: string) => ReactNode
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
  const [query, setQuery] = useState("")
  const [kvQuery, setKvQuery] = useState("")
  const [scopeFilter, setScopeFilter] = useState("all")
  const [sortBy, setSortBy] = useState<FrameSessionSortBy | null>(null)

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
  const scopes = useMemo(() => collectScopes(items), [items])
  const visibleItems = useMemo(
    () => filterAndSortSessions(items, query, scopeFilter, sortBy),
    [items, query, scopeFilter, sortBy],
  )
  const total = sessions.data?.total ?? 0
  const pageCount = Math.max(1, Math.ceil(total / pageSize))
  const activeFilterCount = scopeFilter !== "all" ? 1 : 0
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

  function changeQuery(value: string) {
    setQuery(value)
    setPage(0)
  }

  function changeScopeFilter(value: string) {
    setScopeFilter(value)
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
          <Table className={TRACE_TABLE_CLASSNAME}>
            <TableHeader>
              <TableRow>
                <TableHead className={TRACE_COL1_CLASSNAME}>
                  {t("gatewayFramesColSession")}
                </TableHead>
                <TableHead className={TRACE_COL2_CLASSNAME}>
                  {t("gatewayFramesColTags")}
                </TableHead>
                <TableHead>{t("gatewayFramesColTime")}</TableHead>
                <TableHead>{t("gatewayFramesColRequest")}</TableHead>
                <TableHead>{t("gatewayFramesColResponse")}</TableHead>
                <TableHead>{t("gatewayFramesColCount")}</TableHead>
                {props.onOpenSession ? (
                  <TableHead>{t("gatewayFramesColActions")}</TableHead>
                ) : null}
              </TableRow>
            </TableHeader>
            <TableBody>
              {visibleItems.map((session) => (
                <TableRow key={session.session_id}>
                  <TableCell className={TRACE_COL1_CELL_CLASSNAME} title={session.session_id}>
                    {session.session_id.slice(0, 8)}
                  </TableCell>
                  <TableCell className={TRACE_COL2_CELL_CLASSNAME}>
                    {session.service_name || session.scope ? (
                      <div className={TRACE_COL_TAGS_CLASSNAME}>
                        {session.service_name ? (
                          <Badge variant="secondary" className="shrink-0">
                            {session.service_name}
                          </Badge>
                        ) : null}
                        {session.scope ? (
                          <Badge variant="outline" className="shrink-0">
                            {session.scope}
                          </Badge>
                        ) : null}
                      </div>
                    ) : (
                      <span className="text-xs text-muted-foreground">—</span>
                    )}
                  </TableCell>
                  <TableCell className="text-xs tabular-nums text-muted-foreground">
                    <SessionActiveRange
                      firstMs={session.first_ts_unix_ms}
                      lastMs={session.last_ts_unix_ms}
                    />
                  </TableCell>
                  <TableCell
                    className="text-xs tabular-nums"
                    title={`${session.request_bytes} B`}
                  >
                    {formatByteSize(session.request_bytes)}
                  </TableCell>
                  <TableCell
                    className="text-xs tabular-nums"
                    title={`${session.response_bytes} B`}
                  >
                    {formatByteSize(session.response_bytes)}
                  </TableCell>
                  <TableCell className="text-xs tabular-nums">
                    {session.frame_count}
                  </TableCell>
                  {props.onOpenSession ? (
                    <TableCell>
                      <Button
                        size="sm"
                        variant="link"
                        className="h-auto px-0 text-xs underline"
                        onClick={() => props.onOpenSession?.(session)}
                      >
                        {t("detail")}
                      </Button>
                    </TableCell>
                  ) : null}
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
        {pager}
      </>
    )

  const toolbarActions = (
    <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
      {view === "table" ? (
        <>
          <UiverseSearchInput
            id="frame-session-list-search"
            placeholder={t("searchSessions")}
            value={query}
            onChange={changeQuery}
          />
          <FramesFilterDialog
            activeFilterCount={activeFilterCount}
            onScopeFilterChange={changeScopeFilter}
            onSort={setSortBy}
            scopeFilter={scopeFilter}
            scopes={scopes}
            sortBy={sortBy}
          />
        </>
      ) : null}
      {view === "kv" && props.kvSlot ? (
        <UiverseSearchInput
          id="kv-list-search"
          placeholder={t("searchKeys")}
          value={kvQuery}
          onChange={setKvQuery}
        />
      ) : null}
      <Button disabled={refreshSpinning} size="sm" variant="outline" onClick={refresh}>
        <RefreshCwSpinIcon data-icon="inline-start" spinning={refreshSpinning} />
        {t("refresh")}
      </Button>
    </div>
  )

  const body =
    view === "history" && props.historySlot ? (
      props.historySlot
    ) : view === "kv" && props.kvSlot ? (
      props.kvSlot(kvQuery)
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
              fill ? "shrink-0 border-b px-4 py-2" : undefined,
            )}
          >
            {toolbarActions}
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
                {props.kvSlot ? (
                  <TabsTrigger value="kv" className="flex-none">
                    {t("kv")}
                  </TabsTrigger>
                ) : null}
                {props.historySlot ? (
                  <TabsTrigger value="history" className="flex-none">
                    {t("gatewayFramesTabLog")}
                  </TabsTrigger>
                ) : null}
                <TabsTrigger value="table" className="flex-none">
                  {t("gatewayFramesTabTable")}
                </TabsTrigger>
              </WorkspaceTabsList>
              {toolbarActions}
            </div>

            {props.kvSlot ? (
              <TabsContent value="kv" className={tabContentClassName}>
                {props.kvSlot(kvQuery)}
              </TabsContent>
            ) : null}
            {props.historySlot ? (
              <TabsContent value="history" className={tabContentClassName}>
                {props.historySlot}
              </TabsContent>
            ) : null}
            <TabsContent value="table" className={tabContentClassName}>
              {tableBody}
            </TabsContent>
          </Tabs>
        </PanelCard>
      )}
    </>
  )
}
