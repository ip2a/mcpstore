import { PlusIcon, RefreshCwIcon } from "lucide-react"
import { useMemo, useState } from "react"

import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import {
  breadcrumbUppercase,
  WorkspaceBreadcrumb,
} from "@/components/shared/workspace-page-header"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { UiverseSearchInput } from "@/components/shared/uiverse-search-input"
import { ServiceListCard } from "@/components/shared/service-list-card"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useScopesQuery } from "@/features/agents/queries"
import { useAgentScope } from "@/features/agents/use-agent-scope"
import { AddServiceDialog } from "@/features/services/add-service-dialog"
import { ServiceCardDialog } from "@/features/services/service-card-dialog"
import { ServicesFilterDialog } from "@/features/services/services-filter-dialog"
import { useServicesList } from "@/features/services/use-services-list"
import type { AgentItem, ScopeSummary, ScopeView, ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

type ScopeCardId = "store" | string

function scopeIdFromSummary(summary: ScopeSummary): ScopeCardId {
  return summary.scope.type === "agent" ? summary.scope.agent_id : summary.scope.type
}

function scopeTitleFromSummary(summary: ScopeSummary, t: (key: string) => string): string {
  if (summary.scope.type === "root") return t("filterAll")
  if (summary.scope.type === "store") return t("store")
  if (summary.scope.type === "agent") return summary.scope.agent_id
  return scopeIdFromSummary(summary)
}

function selectedScopeFromId(scopeId: ScopeCardId): ScopeView {
  if (scopeId === "root") return { type: "root" }
  if (scopeId === "store") return { type: "store" }
  return { type: "agent", agent_id: scopeId }
}

export function ServicesView(props: {
  services: ServiceInstance[]
  agents: AgentItem[]
  busy: string | null
  error: string | null
  loading: boolean
  onCache: () => void
  onCheck: () => void
  onConnect: (service: ServiceInstance) => void
  onAdded: () => Promise<void>
  onDelete: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  onOpen: (service: ServiceInstance) => void
  onRefresh: () => void
  onRestart: (service: ServiceInstance) => void
}) {
  const { t } = useI18n()
  const [activeTab, setActiveTab] = useState<ScopeCardId>("root")
  const [selectedScopeId, setSelectedScopeId] = useState<ScopeCardId>("root")
  const [addOpen, setAddOpen] = useState(false)

  const scopesQuery = useScopesQuery()
  const scopes = scopesQuery.data || []

  const selectedScope = useMemo(() => selectedScopeFromId(selectedScopeId), [selectedScopeId])

  const {
    loadAgentScope,
    loadingScopeServices,
    scopeServices,
    scopeServicesError,
    scopeServicesErrorMessage,
    scopeTools,
  } = useAgentScope({ busy: props.busy, selectedScope, services: props.services })

  const {
    applySort,
    filteredServices,
    query,
    setQuery,
    setStatusFilter,
    statusFilter,
  } = useServicesList(scopeServices)

  const statusFilterCount = statusFilter.length

  const scopeToolCountByInstance = useMemo(() => {
    const counts = new Map<string, number>()
    for (const { instance } of scopeTools) {
      counts.set(instance.instance_id, (counts.get(instance.instance_id) || 0) + 1)
    }
    return counts
  }, [scopeTools])

  function selectScopeTab(scopeId: ScopeCardId) {
    setSelectedScopeId(scopeId)
    setActiveTab(scopeId)
  }

  function handleRefresh() {
    void props.onRefresh()
    void loadAgentScope()
  }

  const loadingScope = props.loading || loadingScopeServices
  const scopeTabContentClassName = "mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"

  const selectedScopeTitle = useMemo(() => {
    const summary = scopes.find((item) => scopeIdFromSummary(item) === selectedScopeId)
    if (summary) return scopeTitleFromSummary(summary, t)
    if (selectedScopeId === "root") return t("filterAll")
    if (selectedScopeId === "store") return t("store")
    return selectedScopeId
  }, [scopes, selectedScopeId, t])

  return (
    <>
      <WorkspaceBreadcrumb
        ariaLabel={t("breadcrumb")}
        crumbs={[
          {
            label: t("servicesWorkspaceTitle"),
            onClick: () => selectScopeTab("root"),
          },
          { label: breadcrumbUppercase(selectedScopeTitle) },
        ]}
        subtitle={t("servicesWorkspaceDescription", { count: scopeServices.length })}
      />

      <PanelCard className="h-full min-h-0 flex-1 gap-0 p-0">
        <Tabs
          value={activeTab}
          onValueChange={(value) => selectScopeTab(value)}
          className="flex min-h-0 flex-1 flex-col overflow-hidden"
        >
          <div className="flex w-full min-w-0 shrink-0 items-center gap-3 border-b px-4 py-2">
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
            <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
              <UiverseSearchInput
                id="service-list-search"
                placeholder={t("searchServices")}
                value={query}
                onChange={setQuery}
              />
              <ServicesFilterDialog
                activeFilterCount={statusFilterCount}
                hideScopeFilters
                onSort={applySort}
                onStatusFilterChange={setStatusFilter}
                statusFilter={statusFilter}
              />
              <Button variant="outline" size="sm" onClick={handleRefresh} disabled={loadingScope}>
                <RefreshCwIcon data-icon="inline-start" />
                {t("refresh")}
              </Button>
              <AddServiceDialog
                agents={props.agents}
                onAdded={props.onAdded}
                open={addOpen}
                onOpenChange={setAddOpen}
                showTrigger={false}
                size="sm"
              />
            </div>
          </div>

          {scopes.map((summary) => {
            const scopeId = scopeIdFromSummary(summary)
            return (
              <TabsContent key={scopeId} value={scopeId} className={scopeTabContentClassName}>
                {activeTab === scopeId ? (
                  <ServicesScopeContent
                    busy={props.busy}
                    error={props.error}
                    filteredServices={filteredServices}
                    loading={loadingScope}
                    loadAgentScope={loadAgentScope}
                    onAdd={() => setAddOpen(true)}
                    onConnect={props.onConnect}
                    onDelete={props.onDelete}
                    onDisconnect={props.onDisconnect}
                    onOpen={props.onOpen}
                    onRefresh={handleRefresh}
                    onRestart={props.onRestart}
                    scopeError={scopeServicesError}
                    scopeErrorMessage={scopeServicesErrorMessage}
                    scopeToolCountByInstance={scopeToolCountByInstance}
                    services={props.services}
                  />
                ) : null}
              </TabsContent>
            )
          })}
        </Tabs>
      </PanelCard>
    </>
  )
}

function AddServiceCard({ onSelect }: { onSelect: () => void }) {
  const { t } = useI18n()
  return (
    <div
      role="button"
      tabIndex={0}
      className="group flex min-h-[9.5rem] cursor-pointer flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border/70 bg-card/40 p-4 text-muted-foreground transition-colors hover:border-border hover:bg-muted/40 hover:text-foreground"
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault()
          onSelect()
        }
      }}
    >
      <span className="flex size-8 items-center justify-center rounded-md border border-dashed border-border/70">
        <PlusIcon className="size-4" />
      </span>
      <span className="text-sm font-medium">{t("addService")}</span>
    </div>
  )
}

function ServicesScopeContent({
  busy,
  error,
  filteredServices,
  loading,
  loadAgentScope,
  onAdd,
  onConnect,
  onDelete,
  onDisconnect,
  onOpen,
  onRefresh,
  onRestart,
  scopeError,
  scopeErrorMessage,
  scopeToolCountByInstance,
  services,
}: {
  busy: string | null
  error: string | null
  filteredServices: ServiceInstance[]
  loading: boolean
  loadAgentScope: () => void
  onAdd: () => void
  onConnect: (service: ServiceInstance) => void
  onDelete: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  onOpen: (service: ServiceInstance) => void
  onRefresh: () => void
  onRestart: (service: ServiceInstance) => void
  scopeError: unknown
  scopeErrorMessage: string
  scopeToolCountByInstance: Map<string, number>
  services: ServiceInstance[]
}) {
  const { t } = useI18n()
  const [cardServiceId, setCardServiceId] = useState<string | null>(null)
  const cardService =
    (cardServiceId
      ? filteredServices.find((service) => service.instance_id === cardServiceId) ||
        services.find((service) => service.instance_id === cardServiceId)
      : null) ?? null
  if (scopeError) {
    return (
      <div className="px-4 py-2">
        <PageError
          title={t("agentScopeFailedToLoad")}
          message={scopeErrorMessage}
          onRefresh={loadAgentScope}
        />
      </div>
    )
  }

  if (error && services.length === 0) {
    return (
      <div className="px-4 py-2">
        <PageError title={t("dashboardFailedToLoad")} message={error} onRefresh={onRefresh} />
      </div>
    )
  }

  if (loading && filteredServices.length === 0) {
    return (
      <div className="px-4 py-2">
        <PageSkeleton />
      </div>
    )
  }

  return (
    <>
      <ScrollPane
        className="min-h-0 flex-1"
        innerClassName={cn(
          "px-4 py-3 pb-6",
          "grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4",
        )}
      >
        {filteredServices.map((service) => {
          const toolCount =
            scopeToolCountByInstance.get(service.instance_id) ?? service.tools?.length ?? 0

          return (
            <ServiceListCard
              key={service.instance_id}
              service={service}
              toolCount={toolCount}
              onSelect={() => setCardServiceId(service.instance_id)}
            />
          )
        })}
        <AddServiceCard onSelect={onAdd} />
      </ScrollPane>
      <ServiceCardDialog
        busy={busy}
        open={Boolean(cardService)}
        service={cardService}
        onOpenChange={(open) => {
          if (!open) setCardServiceId(null)
        }}
        onConnect={onConnect}
        onDelete={onDelete}
        onDisconnect={onDisconnect}
        onOpen={onOpen}
        onRestart={onRestart}
        onUpdated={async () => {
          await onRefresh()
        }}
      />
    </>
  )
}
