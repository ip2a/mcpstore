import { useQuery } from "@tanstack/react-query"

import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { HistoryPanel } from "@/features/history/history-view"
import { FrameStreamCard } from "@/features/traces/frame-stream-card"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import { getFrameSessions } from "@/lib/api"

type TracesTab = "table" | "graph" | "history"

/** Insights → Traces: outbound MCP frames + request history. */
export function TracesView({ initialTab }: { initialTab?: TracesTab } = {}) {
  const { t } = useI18n()
  const sessions = useQuery({
    queryKey: queryKeys.gatewayFrameSessionsPage("upstream", 1, 0),
    queryFn: () => getFrameSessions({ face: "upstream", hours: 24, limit: 1, offset: 0 }),
  })

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("tracesWorkspaceTitle")}</h1>}
        subtitle={t("tracesWorkspaceDescription", {
          count: sessions.data?.total ?? 0,
        })}
      />
      <FrameStreamCard
        face="upstream"
        fill
        historySlot={<HistoryPanel />}
        initialView={initialTab}
      />
    </>
  )
}
