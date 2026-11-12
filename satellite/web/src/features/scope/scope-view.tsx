import { useState, useMemo } from "react"
import { ChevronRightIcon, ListChecksIcon } from "lucide-react"

import { UiverseSearchInput } from "@/components/shared/uiverse-search-input"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SERVICE_STATUS_LABEL_KEYS } from "@/components/shared/service-status-badge"
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
import { useToolOverridesQuery } from "@/features/overrides/queries"
import { ToolConfigRow } from "@/features/scope/tool-config-row"
import { ToolPolicyDialog } from "@/features/scope/tool-policy-dialog"
import {
  deriveServiceDisplayStatus,
  type ServiceDisplayStatus,
} from "@/features/services/service-display-status"
import type { ScopeSummary, ScopeView, ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

const SCOPE_STATUS_ORDER: ServiceDisplayStatus[] = [
  "connected",
  "disconnected",
  "error",
  "connecting",
]

const SCOPE_STATUS_COLOR: Record<ServiceDisplayStatus, string> = {
  connected: "text-emerald-500",
  connecting: "text-amber-500",
  disconnected: "text-muted-foreground",
  error: "text-destructive",
}

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
  scopeId?: string
  onScopeIdChange?: (scopeId: string | null) => void
}) {
  const { t } = useI18n()
  const scopesQuery = useScopesQuery()
  const scopes = useMemo(
    () => (scopesQuery.data || []).filter((summary) => summary.scope.type !== "all"),
    [scopesQuery.data],
  )
  const [localScopeId, setLocalScopeId] = useState<string | null>(props.scopeId ?? null)
  const selectedScopeId = props.onScopeIdChange ? (props.scopeId ?? null) : localScopeId
  const setSelectedScopeId = props.onScopeIdChange ?? setLocalScopeId
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

  const loading = scopesQuery.isFetching
  const error = scopesQuery.error
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : "Failed to load scope"

  // List payload already embeds tools[] — no per-service /tools/list fan-out.
  const serviceTools = useMemo(
    () =>
      scopeServices.map((service) => ({
        service,
        tools: service.tools || [],
      })),
    [scopeServices],
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
    await Promise.all([scopesQuery.refetch(), toolOverridesQuery.refetch()])
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
                      services={scoped}
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
  services,
  onSelect,
}: {
  title: string
  services: ServiceInstance[]
  onSelect: () => void
}) {
  const { t } = useI18n()
  const statusCounts = useMemo(() => {
    const counts: Record<ServiceDisplayStatus, number> = {
      connected: 0,
      connecting: 0,
      disconnected: 0,
      error: 0,
    }
    for (const service of services) {
      counts[deriveServiceDisplayStatus(service.state)] += 1
    }
    return SCOPE_STATUS_ORDER.flatMap((status) => {
      const count = counts[status]
      if (count <= 0) return []
      return [{ status, count }]
    })
  }, [services])

  return (
    <div
      role="button"
      tabIndex={0}
      className={cn(
        "group relative z-[1] flex h-[9.5rem] cursor-pointer flex-col overflow-hidden rounded-lg border border-border/50 bg-card p-4 text-center",
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
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-2 px-1">
        <span className="line-clamp-3 font-mono text-xl font-bold leading-snug tracking-tight text-foreground">
          {title}
        </span>
        {statusCounts.length ? (
          <p className="line-clamp-2 text-xs lowercase leading-5">
            {statusCounts.map(({ status, count }, index) => (
              <span key={status}>
                {index > 0 ? <span className="text-muted-foreground"> · </span> : null}
                <span className={SCOPE_STATUS_COLOR[status]}>
                  {count} {t(SERVICE_STATUS_LABEL_KEYS[status])}
                </span>
              </span>
            ))}
          </p>
        ) : (
          <p className="text-xs text-muted-foreground">
            {services.length} {t("services")}
          </p>
        )}
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
