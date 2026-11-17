import { useEffect, useMemo, useState } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import { ArrowDownIcon, ArrowUpIcon } from "lucide-react"

import { PageEmpty, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { WorkspaceBreadcrumb } from "@/components/shared/workspace-page-header"
import { WorkspaceTabsList } from "@/components/shared/workspace-tabs-list"
import {
  WORKSPACE_LIST_INSET_CLASSNAME,
  WORKSPACE_PANEL_CLASSNAME,
  WORKSPACE_TABS_CLASSNAME,
  WORKSPACE_TOOLBAR_CLASSNAME,
} from "@/components/shared/workspace-toolbar"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsTrigger } from "@/components/ui/tabs"
import { getFrameHistory, type FrameRecord } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"

const ALL_TAB = "all"

/** Shared session detail: JSON-RPC frame timeline for one connection. */
export function TraceSessionDetailView(props: {
  sessionId: string
  face: "gateway" | "upstream"
  onBack: () => void
}) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [methodTab, setMethodTab] = useState(ALL_TAB)
  const timeline = useQuery({
    queryKey: queryKeys.gatewayFrameHistory(props.sessionId),
    queryFn: () => getFrameHistory(props.sessionId),
  })

  const frames = timeline.data ?? []
  const methods = useMemo(() => collectMethods(frames), [frames])
  const tabs = useMemo(() => [ALL_TAB, ...methods], [methods])
  const activeTab = tabs.includes(methodTab) ? methodTab : ALL_TAB
  const filtered = useMemo(
    () => framesForMethod(frames, activeTab),
    [frames, activeTab],
  )

  useEffect(() => {
    if (filtered.length === 0) {
      setSelectedId(null)
      return
    }
    if (selectedId != null && filtered.some((frame) => frame.id === selectedId)) return
    setSelectedId(filtered[0]!.id)
  }, [filtered, selectedId])

  const selected = filtered.find((frame) => frame.id === selectedId) ?? null

  const peer =
    frames.find((frame) => frame.peer)?.peer ??
    frames.find((frame) => frame.caller)?.caller ??
    null
  const shortId = props.sessionId.slice(0, 8)
  const faceLabel =
    props.face === "gateway" ? t("gatewayFramesFaceGateway") : t("gatewayFramesFaceUpstream")
  const parentLabel = t("tracesWorkspaceTitle")
  const headerSubtitle = [faceLabel, peer, props.sessionId].filter(Boolean).join(" · ")

  function refresh() {
    void queryClient.invalidateQueries({
      queryKey: queryKeys.gatewayFrameHistory(props.sessionId),
    })
  }

  const tabContentClassName = "mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"

  return (
    <>
      <WorkspaceBreadcrumb
        ariaLabel={t("gatewayFramesDialogTitle", { id: shortId })}
        crumbs={[
          { label: parentLabel, onClick: props.onBack },
          { label: t("gatewayFramesDialogTitle", { id: shortId }) },
        ]}
        subtitle={headerSubtitle}
      />
      <PanelCard className={WORKSPACE_PANEL_CLASSNAME}>
        <Tabs
          value={activeTab}
          onValueChange={setMethodTab}
          className={WORKSPACE_TABS_CLASSNAME}
        >
          <div className={WORKSPACE_TOOLBAR_CLASSNAME}>
            <WorkspaceTabsList value={activeTab}>
              {tabs.map((tab) => (
                <TabsTrigger key={tab} value={tab} title={tab} className="flex-none">
                  {tab === ALL_TAB ? t("filterAll") : tab}
                </TabsTrigger>
              ))}
            </WorkspaceTabsList>
            <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
              <Button
                disabled={timeline.isFetching}
                size="sm"
                variant="outline"
                onClick={refresh}
              >
                <RefreshCwSpinIcon data-icon="inline-start" spinning={timeline.isFetching} />
                {t("refresh")}
              </Button>
            </div>
          </div>

          {tabs.map((tab) => (
            <TabsContent key={tab} value={tab} className={tabContentClassName}>
              {activeTab !== tab ? null : timeline.isLoading ? (
                <PageSkeleton />
              ) : filtered.length === 0 ? (
                <p className={cn(WORKSPACE_LIST_INSET_CLASSNAME, "text-sm text-muted-foreground")}>
                  {t("gatewayFramesNoFrames")}
                </p>
              ) : (
                <div className="flex min-h-0 flex-1 overflow-hidden">
                  <div className="flex w-[min(22rem,42%)] shrink-0 flex-col overflow-hidden border-r">
                    <ScrollPane
                      className="min-h-0 flex-1"
                      innerClassName={cn(WORKSPACE_LIST_INSET_CLASSNAME, "flex flex-col gap-1")}
                    >
                      {filtered.map((frame) => (
                        <FrameTimelineRow
                          frame={frame}
                          key={frame.id}
                          selected={frame.id === selectedId}
                          onSelect={() => setSelectedId(frame.id)}
                        />
                      ))}
                    </ScrollPane>
                  </div>
                  <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
                    {selected ? (
                      <FrameDetailPane frame={selected} />
                    ) : (
                      <PageEmpty title={t("gatewayFramesNoFrames")} />
                    )}
                  </div>
                </div>
              )}
            </TabsContent>
          ))}
        </Tabs>
      </PanelCard>
    </>
  )
}

function collectMethods(frames: FrameRecord[]): string[] {
  const seen = new Set<string>()
  for (const frame of frames) {
    if (frame.method) seen.add(frame.method)
  }
  return [...seen].sort((a, b) => a.localeCompare(b))
}

/** Method tab shows that method plus paired responses/errors (same jsonrpc id). */
function framesForMethod(frames: FrameRecord[], tab: string): FrameRecord[] {
  if (tab === ALL_TAB) return frames
  const ids = new Set(
    frames
      .filter((frame) => frame.method === tab && frame.jsonrpc_id != null)
      .map((frame) => frame.jsonrpc_id!),
  )
  return frames.filter(
    (frame) =>
      frame.method === tab || (frame.jsonrpc_id != null && ids.has(frame.jsonrpc_id)),
  )
}

function FrameTimelineRow({
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

function FrameDetailPane({ frame }: { frame: FrameRecord }) {
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
      className="min-h-0 flex-1"
      innerClassName={cn(WORKSPACE_LIST_INSET_CLASSNAME, "flex flex-col gap-3")}
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
