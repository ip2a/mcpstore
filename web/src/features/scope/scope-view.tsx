import { useState, useMemo } from "react"
import { PencilIcon, ChevronRightIcon, ListChecksIcon } from "lucide-react"

import { UiverseSearchInput } from "@/components/shared/uiverse-search-input"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { Button } from "@/components/ui/button"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { useScopesQuery } from "@/features/agents/queries"
import { useInstanceToolsQueries } from "@/features/tools/queries"
import { useToolOverridesQuery } from "@/features/overrides/queries"
import { ToolConfigRow } from "@/features/scope/tool-config-row"
import { ToolPolicyDialog } from "@/features/scope/tool-policy-dialog"
import type { ScopeSummary, ScopeView, ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

function sameScopeRefView(a: { type: string; agent_id?: string }, scope: ScopeView): boolean {
  if (scope.type === "root") return true
  if (scope.type === "store") return a.type === "store"
  return a.type === "agent" && a.agent_id === scope.agent_id
}

function scopeIdFromSummary(summary: ScopeSummary): string {
  return summary.scope.type === "agent" ? summary.scope.agent_id : summary.scope.type
}

function scopeTitleFromSummary(summary: ScopeSummary, t: (key: string) => string): string {
  if (summary.scope.type === "root") return "root"
  if (summary.scope.type === "store") return t("store")
  if (summary.scope.type === "agent") return summary.scope.agent_id
  return scopeIdFromSummary(summary)
}

export function ScopeView(props: { services: ServiceInstance[] }) {
  const { t } = useI18n()
  const scopesQuery = useScopesQuery()
  const scopes = scopesQuery.data || []
  const [selectedScopeId, setSelectedScopeId] = useState<string>("root")
  const [searchQuery, setSearchQuery] = useState("")
  const toolOverridesQuery = useToolOverridesQuery()
  const toolOverrides = toolOverridesQuery.data?.overrides || []

  const selectedScope: ScopeView = useMemo(() => {
    if (selectedScopeId === "root") return { type: "root" }
    if (selectedScopeId === "store") return { type: "store" }
    return { type: "agent", agent_id: selectedScopeId }
  }, [selectedScopeId])

  const overrideSet = useMemo(() => {
    const set = new Set<string>()
    for (const override of toolOverrides) {
      if (sameScopeRefView(override.scope, selectedScope)) {
        set.add(`${override.service_name}::${override.tool_name}`)
      }
    }
    return set
  }, [toolOverrides, selectedScope])

  const scopeServices = useMemo(() => {
    if (selectedScope.type === "root") return props.services
    if (selectedScope.type === "store") {
      return props.services.filter((service) => service.scope.type === "store")
    }
    return props.services.filter(
      (service) =>
        service.scope.type === "agent" && service.scope.agent_id === selectedScope.agent_id,
    )
  }, [props.services, selectedScope])

  const toolQueries = useInstanceToolsQueries(scopeServices, "all")
  const loading = scopesQuery.isFetching || toolQueries.some((result) => result.isFetching)
  const error = scopesQuery.error || toolQueries.find((result) => result.error)?.error
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : "Failed to load scope"

  const serviceTools = useMemo(
    () =>
      scopeServices.map((service, index) => ({
        service,
        tools: toolQueries[index]?.data || [],
      })),
    [scopeServices, toolQueries],
  )

  const filteredServiceTools = useMemo(() => {
    const needle = searchQuery.trim().toLowerCase()
    if (!needle) return serviceTools

    return serviceTools
      .map(({ service, tools }) => ({
        service,
        tools: tools.filter((tool) => {
          const displayName = tool.title?.trim() || tool.name
          const text = `${displayName} ${tool.description} ${tool.name}`.toLowerCase()
          return text.includes(needle)
        }),
      }))
      .filter(
        ({ service, tools }) =>
          tools.length > 0 || service.service_name.toLowerCase().includes(needle),
      )
  }, [serviceTools, searchQuery])

  async function handleRefresh() {
    await scopesQuery.refetch()
    await Promise.all(toolQueries.map((result) => result.refetch()))
  }

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("scopeConfigurationTitle")}</h1>}
        subtitle={t("scopeConfigurationDescription")}
      />

      <PanelCard className="flex h-full min-h-0 flex-1 flex-col gap-0 overflow-hidden p-0">
        <div className="flex w-full min-w-0 shrink-0 items-center gap-3 border-b px-4 py-2">
          <Tabs value={selectedScopeId} onValueChange={setSelectedScopeId} className="min-w-0">
            <TabsList variant="line" className="h-9 w-fit shrink-0 justify-start">
              {scopes.map((summary) => {
                const scopeId = scopeIdFromSummary(summary)
                const title = scopeTitleFromSummary(summary, t)
                return (
                  <TabsTrigger key={scopeId} value={scopeId} title={title} className="flex-none">
                    {title}
                  </TabsTrigger>
                )
              })}
            </TabsList>
          </Tabs>
          <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
            <UiverseSearchInput
              id="scope-list-search"
              placeholder={t("searchToolsAndServices")}
              value={searchQuery}
              onChange={setSearchQuery}
            />
            <Button size="sm" variant="outline" onClick={handleRefresh} disabled={loading}>
              <RefreshCwSpinIcon data-icon="inline-start" spinning={loading} />
              {t("refresh")}
            </Button>
          </div>
        </div>

        <ScrollPane className="min-h-0 flex-1">
          {error ? (
            <PageError
              title={t("scopeFailedToLoad")}
              message={errorMessage}
              onRefresh={handleRefresh}
            />
          ) : loading && !serviceTools.length ? (
            <PageSkeleton />
          ) : !filteredServiceTools.length ? (
            <div className="flex h-full items-center justify-center p-8 text-center text-muted-foreground">
              {searchQuery ? t("noSearchResults") : t("noServices")}
            </div>
          ) : (
            <div className="space-y-3 p-4">
              {filteredServiceTools.map(({ service, tools }) => (
                <ServiceToolGroup
                  key={service.instance_id}
                  service={service}
                  tools={tools}
                  scope={selectedScope}
                  overrideSet={overrideSet}
                  onPolicyChange={handleRefresh}
                />
              ))}
            </div>
          )}
        </ScrollPane>
      </PanelCard>
    </>
  )
}

function ServiceToolGroup({
  service,
  tools,
  scope,
  overrideSet,
  onPolicyChange,
}: {
  service: ServiceInstance
  tools: Array<{ name: string; description: string; title?: string | null }>
  scope: ScopeView
  overrideSet: Set<string>
  onPolicyChange: () => void
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const [policyDialogOpen, setPolicyDialogOpen] = useState(false)
  const isRootView = scope.type === "root"

  return (
    <>
      <Collapsible open={open} onOpenChange={setOpen}>
        <div className="rounded-lg border border-border/50 bg-card">
          <div className="flex items-center gap-2 px-4 py-3">
            <CollapsibleTrigger asChild>
              <button
                type="button"
                className="flex flex-1 min-w-0 items-center gap-2 text-left transition-colors hover:bg-muted/40 rounded-md -m-1 p-1"
              >
                <ChevronRightIcon
                  className={cn("size-4 shrink-0 transition-transform", open && "rotate-90")}
                />
                <span className="flex-1 font-medium">
                  {service.service_name}
                  {isRootView && service.scope.type === "agent" ? (
                    <span className="ml-2 text-xs text-muted-foreground">
                      ({service.scope.agent_id})
                    </span>
                  ) : null}
                </span>
                <span className="text-sm text-muted-foreground">{tools.length} tools</span>
              </button>
            </CollapsibleTrigger>

            {!isRootView ? (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setPolicyDialogOpen(true)}
                title={t("toolPolicyTitle")}
              >
                <ListChecksIcon className="size-3.5" />
              </Button>
            ) : null}
          </div>

          <CollapsibleContent>
            <div className="border-t border-border/50">
              {isRootView ? (
                <div className="px-4 py-3 text-sm text-muted-foreground">
                  {t("scopeRootReadOnlyHint")}
                </div>
              ) : null}
              {tools.map((tool) => {
                const overrideKey = `${service.service_name}::${tool.name}`
                const hasOverride = overrideSet.has(overrideKey)
                return (
                  <ToolConfigRow
                    key={tool.name}
                    service={service}
                    tool={tool}
                    scope={scope}
                    readOnly={isRootView}
                    hasOverride={hasOverride}
                  />
                )
              })}
            </div>
          </CollapsibleContent>
        </div>
      </Collapsible>

      <ToolPolicyDialog
        open={policyDialogOpen}
        onOpenChange={setPolicyDialogOpen}
        service={service}
        allTools={tools}
        onSuccess={onPolicyChange}
      />
    </>
  )
}
