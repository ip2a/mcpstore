import { ArrowUpDownIcon, RefreshCwIcon } from "lucide-react"
import { useMemo, useState } from "react"

import { CollapsibleSearchBox } from "@/components/shared/collapsible-search-box"
import { ToolListCard } from "@/components/shared/tool-list-card"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import {
  breadcrumbUppercase,
  instanceScopeBreadcrumbLabel,
  WorkspaceBreadcrumb,
} from "@/components/shared/workspace-page-header"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { ToolsFilterDialog } from "@/features/tools/tools-filter-dialog"
import { useToolsRegistry } from "@/features/tools/use-tools-registry"
import { useI18n } from "@/lib/i18n-context"
import type { AgentItem, ServiceInstance, ToolInfo } from "@/lib/api"
import { toolKey } from "@/lib/tool-info"

type ToolSortBy = "name" | "service"

export function ToolsView(props: {
  agents: AgentItem[]
  services: ServiceInstance[]
  onOpenTool: (instanceId: string, toolName: string) => void
}) {
  const { t } = useI18n()
  const [sortOpen, setSortOpen] = useState(false)
  const [sortBy, setSortBy] = useState<ToolSortBy | null>(null)
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

  const sortedTools = useMemo(() => {
    const items = [...visibleTools]
    if (sortBy === "name") {
      items.sort((a, b) => a.tool.name.localeCompare(b.tool.name))
    } else if (sortBy === "service") {
      items.sort((a, b) => {
        const byService = a.instance.service_name.localeCompare(b.instance.service_name)
        return byService !== 0 ? byService : a.tool.name.localeCompare(b.tool.name)
      })
    }
    return items
  }, [sortBy, visibleTools])

  const serviceTabs = useMemo(
    () => [
      { id: "all", label: t("filterAll") },
      ...scopeInstances.map((instance) => ({
        id: instance.instance_id,
        label: instance.service_name,
      })),
    ],
    [scopeInstances, t],
  )

  const selectedInstance = scopeInstances.find((instance) => instance.instance_id === instanceId)
  const serviceTabLabel = breadcrumbUppercase(
    selectedInstance?.service_name ||
      serviceTabs.find((tab) => tab.id === instanceId)?.label ||
      t("filterAll"),
  )
  const isAllServices = instanceId === "all"
  const listCrumbs = isAllServices
    ? [
        { label: t("navTools"), onClick: () => setInstanceId("all") },
        { label: breadcrumbUppercase(t("filterAll")) },
      ]
    : [
        { label: t("navTools"), onClick: () => setInstanceId("all") },
        {
          label: selectedInstance
            ? instanceScopeBreadcrumbLabel(selectedInstance.scope, {
                store: t("store"),
                all: t("filterAll"),
              })
            : breadcrumbUppercase(scope === "agent" ? agentId || t("agent") : t("store")),
          onClick: () => setInstanceId("all"),
        },
        { label: serviceTabLabel },
      ]

  const applySort = (value: ToolSortBy) => {
    setSortBy(value)
    setSortOpen(false)
  }

  const tabContentClassName = "mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"

  return (
    <>
      <WorkspaceBreadcrumb
        ariaLabel={t("breadcrumb")}
        crumbs={listCrumbs}
        subtitle={t("toolRegistryDescription", {
          count: visibleTools.length,
          scope:
            scope === "agent" ? t("agentScopeLabel", { agentId: agentId || "-" }) : t("storeScope"),
        })}
      />

      <PanelCard className="h-full min-h-0 flex-1 gap-0 p-0">
        <Tabs
          value={instanceId}
          onValueChange={setInstanceId}
          className="flex min-h-0 flex-1 flex-col overflow-hidden"
        >
          <div className="flex w-full min-w-0 shrink-0 items-center gap-3 border-b px-4 py-2">
            <TabsList variant="line" className="h-9 w-fit shrink-0 justify-start">
              {serviceTabs.map((tab) => (
                <TabsTrigger key={tab.id} value={tab.id} title={tab.label} className="flex-none">
                  {tab.label}
                </TabsTrigger>
              ))}
            </TabsList>
            <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
              <CollapsibleSearchBox
                inline
                id="tool-list-search"
                placeholder={t("searchTools")}
                value={query}
                onChange={setQuery}
              />
              <Dialog open={sortOpen} onOpenChange={setSortOpen}>
                <DialogTrigger asChild>
                  <Button variant="outline" size="sm">
                    <ArrowUpDownIcon data-icon="inline-start" />
                    {t("sortLabel")}
                  </Button>
                </DialogTrigger>
                <DialogContent className="sm:max-w-sm">
                  <DialogHeader>
                    <DialogTitle>{t("sortLabel")}</DialogTitle>
                    <DialogDescription>{t("sortHint")}</DialogDescription>
                  </DialogHeader>
                  <div className="flex flex-col gap-2">
                    <Button variant="outline" onClick={() => applySort("name")}>
                      {t("sortName")}
                    </Button>
                    <Button variant="outline" onClick={() => applySort("service")}>
                      {t("service")}
                    </Button>
                  </div>
                  <DialogFooter showCloseButton />
                </DialogContent>
              </Dialog>
              <ToolsFilterDialog
                activeFilterCount={activeFilterCount}
                agentId={agentId}
                agentIds={agentIds}
                hideServiceFilter
                instanceId={instanceId}
                onAgentIdChange={setAgentId}
                onClearPolicy={
                  instanceId !== "all"
                    ? () => {
                        const instance = scopeInstances.find(
                          (item) => item.instance_id === instanceId,
                        )
                        if (instance) void clearToolPolicy(instance)
                      }
                    : undefined
                }
                onInstanceIdChange={setInstanceId}
                onScopeChange={setScope}
                onVisibilityFilterChange={setVisibilityFilter}
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

          {serviceTabs.map((tab) => (
            <TabsContent key={tab.id} value={tab.id} className={tabContentClassName}>
              {instanceId === tab.id ? (
                <ToolsTabContent
                  error={error}
                  errorMessage={errorMessage}
                  loading={loading}
                  loadTools={loadTools}
                  onOpenTool={props.onOpenTool}
                  tools={sortedTools}
                />
              ) : null}
            </TabsContent>
          ))}
        </Tabs>
      </PanelCard>
    </>
  )
}

function ToolsTabContent({
  error,
  errorMessage,
  loadTools,
  loading,
  onOpenTool,
  tools,
}: {
  error: unknown
  errorMessage: string
  loadTools: () => void
  loading: boolean
  onOpenTool: (instanceId: string, toolName: string) => void
  tools: Array<{ instance: ServiceInstance; tool: ToolInfo }>
}) {
  const { t } = useI18n()

  if (error) {
    return (
      <div className="px-4 py-2">
        <PageError title={t("toolsFailedToLoad")} message={errorMessage} onRefresh={loadTools} />
      </div>
    )
  }

  if (loading && !tools.length) {
    return (
      <div className="px-4 py-2">
        <PageSkeleton />
      </div>
    )
  }

  if (!tools.length) {
    return (
      <div className="px-4 py-2">
        <PageEmpty
          title={t("noTools")}
          description={t("noToolsScopeDescription")}
          onRefresh={loadTools}
        />
      </div>
    )
  }

  return (
    <ScrollPane
      className="min-h-0 flex-1"
      innerClassName="px-4 py-2 grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4"
    >
      {tools.map(({ instance, tool }) => {
        const key = toolKey(instance.instance_id, tool)
        return (
          <ToolListCard
            key={key}
            instance={instance}
            tool={tool}
            onSelect={() => onOpenTool(instance.instance_id, tool.name)}
          />
        )
      })}
    </ScrollPane>
  )
}
