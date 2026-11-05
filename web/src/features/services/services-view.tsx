import { ArrowUpDownIcon, LayoutGridIcon, ListIcon, PlusIcon, RefreshCwIcon } from "lucide-react"
import { useMemo, useRef, useState, type ComponentRef, type RefObject } from "react"

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { CollapsibleSearchBox } from "@/components/shared/collapsible-search-box"
import { ServiceListCard } from "@/components/shared/service-list-card"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useScopesQuery } from "@/features/agents/queries"
import { useAgentScope } from "@/features/agents/use-agent-scope"
import { AddServiceDialog } from "@/features/services/add-service-dialog"
import { AddServiceScopeDialog } from "@/features/services/add-service-scope-dialog"
import { ServiceCardDialog } from "@/features/services/service-card-dialog"
import { ServiceList } from "@/features/services/service-list"
import { ServicesFilterDialog } from "@/features/services/services-filter-dialog"
import type { ServiceSortBy } from "@/features/services/use-services-list"
import { usePreserveServiceListScroll } from "@/features/services/use-preserve-service-list-scroll"
import { useServicesList } from "@/features/services/use-services-list"
import type { AgentItem, ScopeSummary, ScopeView, ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

type ScopeCardId = "store" | string
type ServicesViewMode = "cards" | "list"

const VIEW_MODE_STORAGE_KEY = "mcpstore.services-view-mode"

function readViewMode(): ServicesViewMode {
  try {
    const value = localStorage.getItem(VIEW_MODE_STORAGE_KEY)
    return value === "list" ? "list" : "cards"
  } catch {
    return "cards"
  }
}

function writeViewMode(mode: ServicesViewMode) {
  try {
    localStorage.setItem(VIEW_MODE_STORAGE_KEY, mode)
  } catch {
    // Ignore storage failures.
  }
}

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
  onDeclareScope: (agentId: string, serviceName: string) => Promise<void>
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
  const [viewMode, setViewMode] = useState<ServicesViewMode>(readViewMode)
  const [sortOpen, setSortOpen] = useState(false)
  const [addOpen, setAddOpen] = useState(false)
  const listScrollRef = useRef<ComponentRef<typeof ScrollPane>>(null)

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
    moveService,
    query,
    setQuery,
    setStatusFilter,
    statusFilter,
  } = useServicesList(scopeServices)

  const statusFilterCount = statusFilter !== "all" ? 1 : 0

  usePreserveServiceListScroll({
    busy: props.busy,
    listRootRef: listScrollRef,
    services: scopeServices,
  })

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

  function handleViewModeChange(mode: ServicesViewMode) {
    setViewMode(mode)
    writeViewMode(mode)
  }

  const sortBy = (value: ServiceSortBy) => {
    applySort(value)
    setSortOpen(false)
  }

  const loadingScope = props.loading || loadingScopeServices
  const scopeTabContentClassName = "mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("servicesWorkspaceTitle")}</h1>}
        subtitle={t("servicesWorkspaceDescription", { count: props.services.length })}
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
              <CollapsibleSearchBox
                inline
                id="service-list-search"
                placeholder={t("searchServices")}
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
                    <Button variant="outline" onClick={() => sortBy("name")}>
                      {t("sortName")}
                    </Button>
                    <Button variant="outline" onClick={() => sortBy("status")}>
                      {t("sortStatus")}
                    </Button>
                    <Button variant="outline" onClick={() => sortBy("tools")}>
                      {t("sortTools")}
                    </Button>
                  </div>
                  <DialogFooter showCloseButton />
                </DialogContent>
              </Dialog>
              <ServicesFilterDialog
                activeFilterCount={statusFilterCount}
                hideScopeFilters
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
                showTrigger={viewMode === "list"}
                size="sm"
              />
              <div className="flex shrink-0 items-center gap-1">
                <Button
                  aria-label={t("servicesViewCards")}
                  aria-pressed={viewMode === "cards"}
                  size="icon-sm"
                  title={t("servicesViewCards")}
                  variant={viewMode === "cards" ? "secondary" : "outline"}
                  onClick={() => handleViewModeChange("cards")}
                >
                  <LayoutGridIcon aria-hidden="true" />
                </Button>
                <Button
                  aria-label={t("servicesViewList")}
                  aria-pressed={viewMode === "list"}
                  size="icon-sm"
                  title={t("servicesViewList")}
                  variant={viewMode === "list" ? "secondary" : "outline"}
                  onClick={() => handleViewModeChange("list")}
                >
                  <ListIcon aria-hidden="true" />
                </Button>
              </div>
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
                    listScrollRef={listScrollRef}
                    loading={loadingScope}
                    loadAgentScope={loadAgentScope}
                    moveService={moveService}
                    onAdd={() => setAddOpen(true)}
                    onConnect={props.onConnect}
                    onDeclareScope={props.onDeclareScope}
                    onDelete={props.onDelete}
                    onDisconnect={props.onDisconnect}
                    onOpen={props.onOpen}
                    onRefresh={handleRefresh}
                    onRestart={props.onRestart}
                    scopeError={scopeServicesError}
                    scopeErrorMessage={scopeServicesErrorMessage}
                    scopeToolCountByInstance={scopeToolCountByInstance}
                    services={props.services}
                    agents={props.agents}
                    viewMode={viewMode}
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
      className="group flex cursor-pointer flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border/70 bg-card/40 p-4 text-muted-foreground transition-colors hover:border-border hover:bg-muted/40 hover:text-foreground"
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
  agents,
  busy,
  error,
  filteredServices,
  listScrollRef,
  loading,
  loadAgentScope,
  moveService,
  onAdd,
  onConnect,
  onDeclareScope,
  onDelete,
  onDisconnect,
  onOpen,
  onRefresh,
  onRestart,
  scopeError,
  scopeErrorMessage,
  scopeToolCountByInstance,
  services,
  viewMode,
}: {
  agents: AgentItem[]
  busy: string | null
  error: string | null
  filteredServices: ServiceInstance[]
  listScrollRef: RefObject<ComponentRef<typeof ScrollPane> | null>
  loading: boolean
  loadAgentScope: () => void
  moveService: (fromId: string, toId: string) => void
  onAdd: () => void
  onConnect: (service: ServiceInstance) => void
  onDeclareScope: (agentId: string, serviceName: string) => Promise<void>
  onDelete: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  onOpen: (service: ServiceInstance) => void
  onRefresh: () => void
  onRestart: (service: ServiceInstance) => void
  scopeError: unknown
  scopeErrorMessage: string
  scopeToolCountByInstance: Map<string, number>
  services: ServiceInstance[]
  viewMode: ServicesViewMode
}) {
  const { t } = useI18n()
  const [cardServiceId, setCardServiceId] = useState<string | null>(null)
  const [scopeService, setScopeService] = useState<ServiceInstance | null>(null)
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

  if (!filteredServices.length && viewMode === "list") {
    return (
      <div className="px-4 py-2">
        <PageEmpty
          title={t("noServices")}
          description={t("noServicesInViewDescription")}
          onRefresh={onRefresh}
        />
      </div>
    )
  }

  if (viewMode === "list") {
    return (
      <ScrollPane ref={listScrollRef} className="min-h-0 flex-1 px-4 py-2">
        <ServiceList
          agents={agents}
          allServices={services}
          busy={busy}
          onConnect={onConnect}
          onDeclareScope={onDeclareScope}
          onDelete={onDelete}
          onDisconnect={onDisconnect}
          onOpen={onOpen}
          onRefresh={onRefresh}
          onRestart={onRestart}
          services={filteredServices}
          onMove={moveService}
        />
      </ScrollPane>
    )
  }

  return (
    <>
      <ScrollPane
        className="min-h-0 flex-1"
        innerClassName={cn(
          "px-4 py-2",
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
        onAddScope={setScopeService}
        onConnect={onConnect}
        onDelete={onDelete}
        onDisconnect={onDisconnect}
        onOpen={onOpen}
        onRestart={onRestart}
        onUpdated={async () => {
          await onRefresh()
        }}
      />
      <AddServiceScopeDialog
        agents={agents}
        allServices={services}
        busy={busy}
        service={scopeService}
        onOpenChange={(open) => {
          if (!open) setScopeService(null)
        }}
        onDeclareScope={onDeclareScope}
      />
    </>
  )
}

function AddServiceCard({ onSelect }: { onSelect: () => void }) {
  const { t } = useI18n()
  return (
    <div
      role="button"
      tabIndex={0}
      className="group flex cursor-pointer flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border/70 bg-card/40 p-4 text-muted-foreground transition-colors hover:border-border hover:bg-muted/40 hover:text-foreground"
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
