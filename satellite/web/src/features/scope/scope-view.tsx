import { useState, useMemo } from "react"
import { BotIcon, ChevronRightIcon, ListChecksIcon, StoreIcon } from "lucide-react"

import { UiverseSearchInput } from "@/components/shared/uiverse-search-input"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import {
  WORKSPACE_LIST_INSET_CLASSNAME,
  WORKSPACE_TOOLBAR_CLASSNAME,
} from "@/components/shared/workspace-toolbar"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { ScrollPane } from "@/components/shared/scroll-pane"
import {
  breadcrumbUppercase,
  WorkspaceBreadcrumb,
  WorkspacePageHeader,
} from "@/components/shared/workspace-page-header"
import { Button } from "@/components/ui/button"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { useScopesQuery } from "@/features/agents/queries"
import { useInstanceToolsQueries } from "@/features/tools/queries"
import { useToolOverridesQuery } from "@/features/overrides/queries"
import { ToolConfigRow } from "@/features/scope/tool-config-row"
import { ToolPolicyDialog } from "@/features/scope/tool-policy-dialog"
import type { ScopeSummary, ScopeView, ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

const SCOPE_LIST_GRID_CLASSNAME =
  "grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4"

function sameScopeRefView(a: { type: string; agent_id?: string }, scope: ScopeView): boolean {
  if (scope.type === "all") return true
  if (scope.type === "store") return a.type === "store"
  return a.type === "agent" && a.agent_id === scope.agent_id
}

function scopeIdFromSummary(summary: ScopeSummary): string {
  return summary.scope.type === "agent" ? summary.scope.agent_id : summary.scope.type
}

function scopeTitleFromSummary(summary: ScopeSummary, t: (key: string) => string): string {
  if (summary.scope.type === "all") return "all"
  if (summary.scope.type === "store") return t("store")
  if (summary.scope.type === "agent") return summary.scope.agent_id
  return scopeIdFromSummary(summary)
}

function scopeViewFromId(scopeId: string): ScopeView {
  if (scopeId === "all") return { type: "all" }
  if (scopeId === "store") return { type: "store" }
  return { type: "agent", agent_id: scopeId }
}

function servicesForScope(
  services: ServiceInstance[],
  scope: ScopeView,
): ServiceInstance[] {
  if (scope.type === "store") {
    return services.filter((service) => service.scope.type === "store")
  }
  if (scope.type === "agent") {
    return services.filter(
      (service) =>
        service.scope.type === "agent" && service.scope.agent_id === scope.agent_id,
    )
  }
  return services
}

export function ScopeView(props: {
  services: ServiceInstance[]
}) {
  const { t } = useI18n()
  const scopesQuery = useScopesQuery()
  const scopes = useMemo(
    () => (scopesQuery.data || []).filter((summary) => summary.scope.type !== "all"),
    [scopesQuery.data],
  )
  const [selectedScopeId, setSelectedScopeId] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState("")
  const toolOverridesQuery = useToolOverridesQuery()
  const toolOverrides = toolOverridesQuery.data?.overrides || []

  const selectedScope: ScopeView | null = useMemo(
    () => (selectedScopeId ? scopeViewFromId(selectedScopeId) : null),
    [selectedScopeId],
  )

  const overrideSet = useMemo(() => {
    const set = new Set<string>()
    if (!selectedScope) return set
    for (const override of toolOverrides) {
      if (sameScopeRefView(override.scope, selectedScope)) {
        set.add(`${override.service_name}::${override.tool_name}`)
      }
    }
    return set
  }, [toolOverrides, selectedScope])

  const scopeServices = useMemo(() => {
    if (!selectedScope) return []
    if (selectedScope.type === "all") return props.services
    if (selectedScope.type === "store") {
      return props.services.filter((service) => service.scope.type === "store")
    }
    return props.services.filter(
      (service) =>
        service.scope.type === "agent" && service.scope.agent_id === selectedScope.agent_id,
    )
  }, [props.services, selectedScope])

  const toolQueries = useInstanceToolsQueries(scopeServices, "all")
  const detailLoading = toolQueries.some((result) => result.isFetching)
  const loading = scopesQuery.isFetching || (selectedScope !== null && detailLoading)
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

  const selectedSummary = scopes.find((summary) => scopeIdFromSummary(summary) === selectedScopeId)
  const selectedTitle = selectedSummary
    ? breadcrumbUppercase(scopeTitleFromSummary(selectedSummary, t))
    : ""

  async function handleRefresh() {
    await scopesQuery.refetch()
    if (selectedScope) {
      await Promise.all(toolQueries.map((result) => result.refetch()))
    }
  }

  function backToOverview() {
    setSelectedScopeId(null)
    setSearchQuery("")
  }

  return (
    <>
      {selectedScope ? (
        <WorkspaceBreadcrumb
          ariaLabel={t("scopeConfigurationTitle")}
          subtitle={t("scopeConfigurationDescription")}
          crumbs={[
            {
              label: breadcrumbUppercase(t("allScopes")),
              onClick: backToOverview,
            },
            { label: selectedTitle },
          ]}
        />
      ) : (
        <WorkspacePageHeader
          title={<h1 className="text-lg font-semibold">{t("scopeConfigurationTitle")}</h1>}
          subtitle={t("allScopesDescription")}
        />
      )}

      <PanelCard className="flex h-full min-h-0 flex-1 flex-col gap-0 overflow-hidden p-0">
        {/* Mirror tools Tabs: default `gap-2` between toolbar and content + content `py-2` */}
        <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-hidden">
          <div className={cn(WORKSPACE_TOOLBAR_CLASSNAME, "justify-end")}>
            {/* h-9 matches tools TabsList so toolbar height stays aligned */}
            <div className="flex h-9 min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
              {selectedScope ? (
                <UiverseSearchInput
                  id="scope-list-search"
                  placeholder={t("searchToolsAndServices")}
                  value={searchQuery}
                  onChange={setSearchQuery}
                />
              ) : null}
              <Button size="sm" variant="outline" onClick={handleRefresh} disabled={loading}>
                <RefreshCwSpinIcon data-icon="inline-start" spinning={loading} />
                {t("refresh")}
              </Button>
            </div>
          </div>

          <ScrollPane
            className="min-h-0 flex-1"
            innerClassName={
              !selectedScope && scopes.length > 0
                ? cn(WORKSPACE_LIST_INSET_CLASSNAME, SCOPE_LIST_GRID_CLASSNAME)
                : undefined
            }
          >
            {error ? (
              <PageError
                title={t("scopeFailedToLoad")}
                message={errorMessage}
                onRefresh={handleRefresh}
              />
            ) : !selectedScope ? (
              scopesQuery.isFetching && !scopes.length ? (
                <PageSkeleton />
              ) : !scopes.length ? (
                <PageEmpty title={t("noScopes")} description={t("noScopesDescription")} />
              ) : (
                scopes.map((summary) => {
                  const scopeId = scopeIdFromSummary(summary)
                  const title = scopeTitleFromSummary(summary, t)
                  const scoped = servicesForScope(props.services, summary.scope)
                  return (
                    <ScopeListCard
                      key={scopeId}
                      title={title}
                      scope={summary.scope}
                      serviceCount={scoped.length}
                      onSelect={() => setSelectedScopeId(scopeId)}
                    />
                  )
                })
              )
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
        </div>
      </PanelCard>
    </>
  )
}

function ScopeListCard({
  title,
  scope,
  serviceCount,
  onSelect,
}: {
  title: string
  scope: ScopeView
  serviceCount: number
  onSelect: () => void
}) {
  const { t } = useI18n()
  const scopeKind = scope.type === "store" ? t("store") : "agent"
  const Icon = scope.type === "store" ? StoreIcon : BotIcon

  return (
    <div
      role="button"
      tabIndex={0}
      className={cn(
        "group relative z-[1] flex h-[9.5rem] cursor-pointer flex-col overflow-hidden rounded-lg border border-border/50 bg-card p-4 text-left",
        "shadow-[0_14px_28px_-20px_rgba(59,130,246,0.28)] transition-[border-color,background-color,box-shadow] duration-[400ms]",
        "hover:z-10 hover:border-sky-500/25 hover:bg-sky-50/90 hover:shadow-none dark:hover:bg-sky-950/40",
        "dark:shadow-[0_14px_28px_-20px_rgba(125,211,252,0.18)]",
      )}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault()
          onSelect()
        }
      }}
    >
      <div className="flex min-h-0 flex-1 flex-col transition-all duration-[400ms] group-hover:pointer-events-none group-hover:opacity-0">
        <div className="flex min-w-0 items-start gap-2">
          <span
            aria-hidden
            className="flex size-8 shrink-0 items-center justify-center rounded-md border border-border/50 bg-muted/60 text-muted-foreground"
          >
            <Icon className="size-4" />
          </span>
          <span className="min-w-0 flex-1">
            <span className="line-clamp-1 font-mono text-sm font-semibold leading-snug">{title}</span>
            <span className="mt-1 flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
              <span className="truncate">{scopeKind}</span>
            </span>
          </span>
        </div>

        <div className="mt-3 min-h-10 min-w-0 w-full text-xs leading-5 text-muted-foreground">
          <p className="truncate">
            {serviceCount} {t("services")}
          </p>
          <p className="truncate">
            by <span className="text-[#0077c2]">{scopeKind}</span>
          </p>
        </div>
      </div>

      <div
        aria-hidden
        className={cn(
          "pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-2 px-5",
          "opacity-0 transition-all duration-[400ms] [transform:rotate(90deg)_scaleX(-1)]",
          "group-hover:opacity-100 group-hover:[transform:rotate(0deg)_scaleX(1)]",
        )}
      >
        <span className="line-clamp-3 text-center font-mono text-xl font-bold leading-snug tracking-tight text-foreground">
          {title}
        </span>
        <span className="text-xs text-muted-foreground">
          by <span className="text-[#0077c2]">{scopeKind}</span>
        </span>
      </div>
    </div>
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
  const isAllView = scope.type === "all"

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
                  {isAllView && service.scope.type === "agent" ? (
                    <span className="ml-2 text-xs text-muted-foreground">
                      ({service.scope.agent_id})
                    </span>
                  ) : null}
                </span>
                <span className="text-sm text-muted-foreground">{tools.length} tools</span>
              </button>
            </CollapsibleTrigger>

            {!isAllView ? (
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
              {isAllView ? (
                <div className="px-4 py-3 text-sm text-muted-foreground">
                  {t("scopeAllReadOnlyHint")}
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
                    readOnly={isAllView}
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
