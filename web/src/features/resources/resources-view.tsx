import { useMemo, useState } from "react"

import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { ResourceListCard } from "@/components/shared/resource-list-card"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import {
  WORKSPACE_TABS_CLASSNAME,
  WORKSPACE_TABS_LIST_CLASSNAME,
  WORKSPACE_TOOLBAR_CLASSNAME,
} from "@/components/shared/workspace-toolbar"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { UiverseSearchInput } from "@/components/shared/uiverse-search-input"
import {
  breadcrumbUppercase,
  instanceScopeBreadcrumbLabel,
  WorkspaceBreadcrumb,
} from "@/components/shared/workspace-page-header"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import {
  McpRegistryFilterDialog,
  type RegistrySortBy,
} from "@/features/mcp-registry/mcp-registry-filter-dialog"
import { useResourcesRegistry } from "@/features/resources/use-resources-registry"
import { useI18n } from "@/lib/i18n-context"
import type { AgentItem, ResourceInfo, ServiceInstance } from "@/lib/api"

export function ResourcesView(props: {
  agents: AgentItem[]
  services: ServiceInstance[]
  onOpenResource: (instanceId: string, resourceUri: string) => void
}) {
  const { t } = useI18n()
  const [sortBy, setSortBy] = useState<RegistrySortBy | null>(null)
  const {
    activeFilterCount,
    agentId,
    agentIds,
    error,
    errorMessage,
    instanceId,
    loadResources,
    loading,
    query,
    scope,
    scopeInstances,
    setAgentId,
    setInstanceId,
    setQuery,
    setScope,
    visibleResources,
  } = useResourcesRegistry({ agents: props.agents, services: props.services })

  const sortedResources = useMemo(() => {
    const items = [...visibleResources]
    if (sortBy === "name") {
      items.sort((a, b) => a.resource.name.localeCompare(b.resource.name))
    } else if (sortBy === "service") {
      items.sort((a, b) => {
        const byService = a.instance.service_name.localeCompare(b.instance.service_name)
        return byService !== 0 ? byService : a.resource.name.localeCompare(b.resource.name)
      })
    }
    return items
  }, [sortBy, visibleResources])

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
        { label: t("navResources"), onClick: () => setInstanceId("all") },
        { label: breadcrumbUppercase(t("filterAll")) },
      ]
    : [
        { label: t("navResources"), onClick: () => setInstanceId("all") },
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

  const tabContentClassName = "mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"

  return (
    <>
      <WorkspaceBreadcrumb
        ariaLabel={t("breadcrumb")}
        crumbs={listCrumbs}
        subtitle={t("resourceRegistryDescription", {
          count: visibleResources.length,
          scope:
            scope === "agent" ? t("agentScopeLabel", { agentId: agentId || "-" }) : t("storeScope"),
        })}
      />

      <PanelCard className="h-full min-h-0 flex-1 gap-0 p-0">
        <Tabs
          value={instanceId}
          onValueChange={setInstanceId}
          className={WORKSPACE_TABS_CLASSNAME}
        >
          <div className={WORKSPACE_TOOLBAR_CLASSNAME}>
            <TabsList variant="line" className={WORKSPACE_TABS_LIST_CLASSNAME}>
              {serviceTabs.map((tab) => (
                <TabsTrigger key={tab.id} value={tab.id} title={tab.label} className="flex-none">
                  {tab.label}
                </TabsTrigger>
              ))}
            </TabsList>
            <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
              <UiverseSearchInput
                id="resource-list-search"
                placeholder={t("searchResources")}
                value={query}
                onChange={setQuery}
              />
              <McpRegistryFilterDialog
                activeFilterCount={activeFilterCount}
                agentId={agentId}
                agentIds={agentIds}
                hideServiceFilter
                instanceId={instanceId}
                onAgentIdChange={setAgentId}
                onInstanceIdChange={setInstanceId}
                onScopeChange={setScope}
                onSort={setSortBy}
                scope={scope}
                scopeInstances={scopeInstances}
                sortBy={sortBy}
              />
              <Button size="sm" variant="outline" onClick={loadResources} disabled={loading}>
                <RefreshCwSpinIcon data-icon="inline-start" spinning={loading} />
                {t("refresh")}
              </Button>
            </div>
          </div>

          {serviceTabs.map((tab) => (
            <TabsContent key={tab.id} value={tab.id} className={tabContentClassName}>
              {instanceId === tab.id ? (
                <ResourcesTabContent
                  error={error}
                  errorMessage={errorMessage}
                  loading={loading}
                  loadResources={loadResources}
                  onOpenResource={props.onOpenResource}
                  resources={sortedResources}
                />
              ) : null}
            </TabsContent>
          ))}
        </Tabs>
      </PanelCard>
    </>
  )
}

function ResourcesTabContent({
  error,
  errorMessage,
  loadResources,
  loading,
  onOpenResource,
  resources,
}: {
  error: unknown
  errorMessage: string
  loadResources: () => void
  loading: boolean
  onOpenResource: (instanceId: string, resourceUri: string) => void
  resources: Array<{ instance: ServiceInstance; resource: ResourceInfo }>
}) {
  const { t } = useI18n()

  if (error) {
    return (
      <div className="px-4 py-2">
        <PageError
          title={t("resourcesFailedToLoad")}
          message={errorMessage}
          onRefresh={loadResources}
        />
      </div>
    )
  }

  if (loading && !resources.length) {
    return (
      <div className="px-4 py-2">
        <PageSkeleton />
      </div>
    )
  }

  if (!resources.length) {
    return (
      <div className="px-4 py-2">
        <PageEmpty
          title={t("noResourcesFound")}
          description={t("noResourcesScopeDescription")}
          onRefresh={loadResources}
        />
      </div>
    )
  }

  return (
    <ScrollPane
      className="min-h-0 flex-1"
      innerClassName="px-4 py-2 grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4"
    >
      {resources.map(({ instance, resource }) => (
        <ResourceListCard
          key={`${instance.instance_id}:${resource.uri || resource.name}`}
          instance={instance}
          resource={resource}
          onSelect={() => onOpenResource(instance.instance_id, resource.uri)}
        />
      ))}
    </ScrollPane>
  )
}
