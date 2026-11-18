import { useEffect, useState, type ReactNode } from "react"

import { useIsFetching, useQueryClient } from "@tanstack/react-query"

import { PanelCard } from "@/components/shared/panel-card"
import { UiverseSearchInput } from "@/components/shared/uiverse-search-input"
import { WORKSPACE_TOOLBAR_CLASSNAME } from "@/components/shared/workspace-toolbar"
import { WorkspaceTabsList } from "@/components/shared/workspace-tabs-list"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsTrigger } from "@/components/ui/tabs"
import { FrameSessionsPanel } from "@/features/traces/frame-sessions-panel"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"
import type { FrameSessionSummary } from "@/lib/api"

export type FrameViewMode =
  | "sessions"
  | "history"
  | "kv"
  | "calls"
  | "projection"

/** Normalize legacy inbound/outbound/table bookmarks to the unified sessions tab. */
export function normalizeFrameView(view?: string | null): FrameViewMode | undefined {
  if (view === "table" || view === "inbound" || view === "outbound") return "sessions"
  if (
    view === "sessions" ||
    view === "history" ||
    view === "kv" ||
    view === "calls" ||
    view === "projection"
  ) {
    return view
  }
  return undefined
}

/** Traces workspace: sessions + optional history / routed calls / projection / KV. */
export function FrameStreamCard(props: {
  /** Fill parent height (Tools-style); table scrolls inside, page chrome stays put. */
  fill?: boolean
  /** Optional History tab content. */
  historySlot?: ReactNode
  /** Optional Routed-calls tab content. */
  callsSlot?: ReactNode
  /** Optional Projection-health tab content. */
  projectionSlot?: ReactNode
  /** Optional KV tab content. */
  kvSlot?: (search: string) => ReactNode
  initialView?: FrameViewMode | "table" | "inbound" | "outbound"
  /** Open shared session detail page. */
  onOpenSession?: (session: FrameSessionSummary) => void
}) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const fill = props.fill ?? false
  const [view, setView] = useState<FrameViewMode>(
    normalizeFrameView(props.initialView) ?? "sessions",
  )
  const [kvQuery, setKvQuery] = useState("")

  useEffect(() => {
    setView(normalizeFrameView(props.initialView) ?? "sessions")
  }, [props.initialView])

  const historyFetching = useIsFetching({ queryKey: queryKeys.gatewayHistory }) > 0
  const projectionFetching = useIsFetching({ queryKey: queryKeys.gatewayProjection }) > 0
  const kvHealthFetching = useIsFetching({ queryKey: queryKeys.kvHealth })
  const kvInspectFetching = useIsFetching({ queryKey: queryKeys.kvInspect })
  const kvFetching = kvHealthFetching + kvInspectFetching > 0
  const sessionsFetching = useIsFetching({ queryKey: queryKeys.gatewayFrameSessions }) > 0

  function refresh() {
    if (view === "history") {
      void queryClient.invalidateQueries({ queryKey: queryKeys.gatewayHistory })
      return
    }
    if (view === "projection") {
      void queryClient.invalidateQueries({ queryKey: queryKeys.gatewayProjection })
      return
    }
    if (view === "kv") {
      void queryClient.invalidateQueries({ queryKey: queryKeys.kvHealth })
      void queryClient.invalidateQueries({ queryKey: queryKeys.kvInspect })
      return
    }
    if (view === "sessions") {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.gatewayFrameSessions,
      })
      return
    }
    // Routed calls long-poll themselves; nothing to invalidate.
  }

  const refreshSpinning =
    view === "history"
      ? historyFetching
      : view === "projection"
        ? projectionFetching
        : view === "kv"
          ? kvFetching
          : view === "sessions"
            ? sessionsFetching
            : false

  const tabContentClassName = fill
    ? "mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
    : "mt-0 flex flex-col gap-2"

  return (
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
            {props.callsSlot ? (
              <TabsTrigger value="calls" className="flex-none">
                {t("gatewayCallsLogTitle")}
              </TabsTrigger>
            ) : null}
            {props.historySlot ? (
              <TabsTrigger value="history" className="flex-none">
                {t("gatewayFramesTabLog")}
              </TabsTrigger>
            ) : null}
            {props.projectionSlot ? (
              <TabsTrigger value="projection" className="flex-none">
                {t("gatewayProjectionTitle")}
              </TabsTrigger>
            ) : null}
            <TabsTrigger value="sessions" className="flex-none">
              {t("gatewayFramesTabSessions")}
            </TabsTrigger>
          </WorkspaceTabsList>
          <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
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
        </div>

        {props.kvSlot ? (
          <TabsContent value="kv" className={tabContentClassName}>
            {props.kvSlot(kvQuery)}
          </TabsContent>
        ) : null}
        {props.callsSlot ? (
          <TabsContent value="calls" className={tabContentClassName}>
            {props.callsSlot}
          </TabsContent>
        ) : null}
        {props.historySlot ? (
          <TabsContent value="history" className={tabContentClassName}>
            {props.historySlot}
          </TabsContent>
        ) : null}
        {props.projectionSlot ? (
          <TabsContent value="projection" className={tabContentClassName}>
            {props.projectionSlot}
          </TabsContent>
        ) : null}
        <TabsContent value="sessions" className={tabContentClassName}>
          <FrameSessionsPanel onOpenSession={props.onOpenSession} />
        </TabsContent>
      </Tabs>
    </PanelCard>
  )
}
