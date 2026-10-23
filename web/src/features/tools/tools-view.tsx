import { RefreshCwIcon } from "lucide-react"

import { ToolListCard } from "@/components/shared/tool-list-card"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Button } from "@/components/ui/button"
import { ToolsFilterDialog } from "@/features/tools/tools-filter-dialog"
import { useToolsRegistry } from "@/features/tools/use-tools-registry"
import { useI18n } from "@/lib/i18n-context"
import { type AgentItem, type ServiceInstance } from "@/lib/api"
import { toolKey } from "@/lib/tool-info"

export function ToolsView(props: {
  agents: AgentItem[]
  services: ServiceInstance[]
  onOpenTool: (instanceId: string, toolName: string) => void
}) {
  const { t } = useI18n()
  const {
    activeFilterCount,
    agentId,
    agentIds,
    error,
    errorMessage,
    loadTools,
    loading,
    query,
    scope,
    instanceId,
    scopeInstances,
    setAgentId,
    setQuery,
    setScope,
    setInstanceId,
    clearToolPolicy,
    setVisibilityFilter,
    visibilityFilter,
    visibleTools,
  } = useToolsRegistry({ agents: props.agents, services: props.services })

  return (
    <>
      <div className="min-w-0 shrink-0 pl-4">
        <h1 className="truncate text-lg font-semibold">{t("toolRegistryTitle")}</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {t("toolRegistryDescription", {
            count: visibleTools.length,
            scope: scope === "agent" ? t("agentScopeLabel", { agentId: agentId || "-" }) : t("storeScope"),
          })}
        </p>
      </div>

      <PanelCard className="@container flex h-full min-h-0 flex-col">
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
        <div className="flex min-w-0 items-center justify-end gap-2">
          <div className="flex shrink-0 items-center gap-2">
            <ToolsFilterDialog
              compact
              activeFilterCount={activeFilterCount}
              agentId={agentId}
              agentIds={agentIds}
              instanceId={instanceId}
              onAgentIdChange={setAgentId}
              onClearPolicy={
                instanceId !== "all"
                  ? () => {
                      const instance = scopeInstances.find((item) => item.instance_id === instanceId)
                      if (instance) void clearToolPolicy(instance)
                    }
                  : undefined
              }
              onInstanceIdChange={setInstanceId}
              onQueryChange={setQuery}
              onScopeChange={setScope}
              onVisibilityFilterChange={setVisibilityFilter}
              query={query}
              scope={scope}
              scopeInstances={scopeInstances}
              visibilityFilter={visibilityFilter}
            />
            <Button size="sm" variant="outline" onClick={loadTools} disabled={loading}>
              <RefreshCwIcon data-icon="inline-start" />
              {t("refresh")}
            </Button>
          </div>
        </div>
        {error ? (
          <PageError title={t("toolsFailedToLoad")} message={errorMessage} onRefresh={loadTools} />
        ) : loading && !visibleTools.length ? (
          <PageSkeleton />
        ) : visibleTools.length ? (
          <ScrollPane
            className="flex-1"
            innerClassName="grid auto-rows-fr grid-cols-[repeat(auto-fill,minmax(min(100%,11rem),1fr))] gap-3"
          >
            {visibleTools.map(({ instance, tool }) => {
              const key = toolKey(instance.instance_id, tool)
              return (
                <ToolListCard
                  key={key}
                  instance={instance}
                  tool={tool}
                  onSelect={() => props.onOpenTool(instance.instance_id, tool.name)}
                />
              )
            })}
          </ScrollPane>
        ) : (
          <PageEmpty title={t("noTools")} description={t("noToolsScopeDescription")} onRefresh={loadTools} />
        )}
      </div>
      </PanelCard>
    </>
  )
}
