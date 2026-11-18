import { useEffect, useMemo, useState } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"

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
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsTrigger } from "@/components/ui/tabs"
import {
  FrameDetailPane,
  FrameTimelineRow,
} from "@/features/traces/frame-session-panes"
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
