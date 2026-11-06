import { useMemo, useState } from "react"

import { PromptListCard } from "@/components/shared/prompt-list-card"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
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
import { usePromptsRegistry } from "@/features/prompts/use-prompts-registry"
import { useI18n } from "@/lib/i18n-context"
import type { AgentItem, PromptInfo, ServiceInstance } from "@/lib/api"

export function PromptsView(props: {
  agents: AgentItem[]
  services: ServiceInstance[]
  onOpenPrompt: (instanceId: string, promptName: string) => void
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
    loadPrompts,
    loading,
    query,
    scope,
    scopeInstances,
    setAgentId,
    setInstanceId,
    setQuery,
    setScope,
    visiblePrompts,
  } = usePromptsRegistry({ agents: props.agents, services: props.services })

  const sortedPrompts = useMemo(() => {
    const items = [...visiblePrompts]
    if (sortBy === "name") {
      items.sort((a, b) => a.prompt.name.localeCompare(b.prompt.name))
    } else if (sortBy === "service") {
      items.sort((a, b) => {
        const byService = a.instance.service_name.localeCompare(b.instance.service_name)
        return byService !== 0 ? byService : a.prompt.name.localeCompare(b.prompt.name)
      })
    }
    return items
  }, [sortBy, visiblePrompts])

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
        { label: t("navPrompts"), onClick: () => setInstanceId("all") },
        { label: breadcrumbUppercase(t("filterAll")) },
      ]
    : [
        { label: t("navPrompts"), onClick: () => setInstanceId("all") },
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
        subtitle={t("promptRegistryDescription", {
          count: visiblePrompts.length,
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
              <UiverseSearchInput
                id="prompt-list-search"
                placeholder={t("searchPrompts")}
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
              <Button size="sm" variant="outline" onClick={loadPrompts} disabled={loading}>
                <RefreshCwSpinIcon data-icon="inline-start" spinning={loading} />
                {t("refresh")}
              </Button>
            </div>
          </div>

          {serviceTabs.map((tab) => (
            <TabsContent key={tab.id} value={tab.id} className={tabContentClassName}>
              {instanceId === tab.id ? (
                <PromptsTabContent
                  error={error}
                  errorMessage={errorMessage}
                  loading={loading}
                  loadPrompts={loadPrompts}
                  onOpenPrompt={props.onOpenPrompt}
                  prompts={sortedPrompts}
                />
              ) : null}
            </TabsContent>
          ))}
        </Tabs>
      </PanelCard>
    </>
  )
}

function PromptsTabContent({
  error,
  errorMessage,
  loadPrompts,
  loading,
  onOpenPrompt,
  prompts,
}: {
  error: unknown
  errorMessage: string
  loadPrompts: () => void
  loading: boolean
  onOpenPrompt: (instanceId: string, promptName: string) => void
  prompts: Array<{ instance: ServiceInstance; prompt: PromptInfo }>
}) {
  const { t } = useI18n()

  if (error) {
    return (
      <div className="px-4 py-2">
        <PageError
          title={t("promptsFailedToLoad")}
          message={errorMessage}
          onRefresh={loadPrompts}
        />
      </div>
    )
  }

  if (loading && !prompts.length) {
    return (
      <div className="px-4 py-2">
        <PageSkeleton />
      </div>
    )
  }

  if (!prompts.length) {
    return (
      <div className="px-4 py-2">
        <PageEmpty
          title={t("noPromptsFound")}
          description={t("noPromptsScopeDescription")}
          onRefresh={loadPrompts}
        />
      </div>
    )
  }

  return (
    <ScrollPane
      className="min-h-0 flex-1"
      innerClassName="px-4 py-2 grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4"
    >
      {prompts.map(({ instance, prompt }) => (
        <PromptListCard
          key={`${instance.instance_id}:${prompt.name}`}
          instance={instance}
          prompt={prompt}
          onSelect={() => onOpenPrompt(instance.instance_id, prompt.name)}
        />
      ))}
    </ScrollPane>
  )
}
