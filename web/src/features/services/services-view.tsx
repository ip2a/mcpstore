import { ActivityIcon, DatabaseIcon } from "lucide-react"

import { HomeHero } from "@/components/home-hero"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SearchBox } from "@/components/shared/search-box"
import { Button } from "@/components/ui/button"
import { ServiceList } from "@/features/services/service-list"
import { ServicesFilterDialog } from "@/features/services/services-filter-dialog"
import { useServicesList } from "@/features/services/use-services-list"
import type { AgentItem, CacheBackend, ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"

export function ServicesView(props: {
  services: ServiceInstance[]
  agents: AgentItem[]
  backend?: CacheBackend
  busy: string | null
  error: string | null
  loading: boolean
  onCache: () => void
  onCheck: () => void
  onConnect: (service: ServiceInstance) => void
  onDelete: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  onOpen: (service: ServiceInstance) => void
  onRefresh: () => void
  onRestart: (service: ServiceInstance) => void
}) {
  const { t } = useI18n()
  const {
    activeFilterCount,
    agentFilter,
    filteredServices,
    query,
    scopeFilter,
    setAgentFilter,
    setQuery,
    setScopeFilter,
    setSortBy,
    setStatusFilter,
    sortBy,
    statusFilter,
    totals,
  } = useServicesList(props.services)
  const agentIds = props.agents.map((agent) => agent.agent_id)

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3 overflow-hidden">
      <HomeHero
        backend={props.backend}
        stats={{
          loading: props.loading,
          services: totals.services,
          connecting: totals.starting,
          agents: agentIds.length,
        }}
      />

      <PanelCard className="min-h-0">
        <div className="@container shrink-0">
          <div className="grid w-full min-w-0 grid-cols-1 items-center gap-2 @min-[28rem]:grid-cols-2 @min-[42rem]:grid-cols-[minmax(0,1fr)_auto_auto_auto] @min-[42rem]:justify-end">
            <div className="min-w-0 @min-[28rem]:col-span-2 @min-[42rem]:col-span-1">
              <SearchBox placeholder={t("searchServices")} value={query} onChange={setQuery} />
            </div>
            <ServicesFilterDialog
              activeFilterCount={activeFilterCount}
              agentFilter={agentFilter}
              agentIds={agentIds}
              onAgentFilterChange={setAgentFilter}
              onScopeFilterChange={setScopeFilter}
              onSortByChange={setSortBy}
              onStatusFilterChange={setStatusFilter}
              scopeFilter={scopeFilter}
              sortBy={sortBy}
              statusFilter={statusFilter}
            />
            <Button variant="outline" onClick={props.onCache}>
              <DatabaseIcon data-icon="inline-start" />
              {t("cache")}
            </Button>
            <Button variant="outline" onClick={props.onCheck} disabled={Boolean(props.busy)}>
              <ActivityIcon data-icon="inline-start" />
              {t("inspect")}
            </Button>
          </div>
        </div>
        <ScrollPane className="min-h-0 flex-1">
          {props.error ? (
            <PageError title={t("dashboardFailedToLoad")} message={props.error} onRefresh={props.onRefresh} />
          ) : props.loading && props.services.length === 0 ? (
            <PageSkeleton />
          ) : filteredServices.length ? (
            <ServiceList {...props} services={filteredServices} />
          ) : (
            <PageEmpty title={t("noServices")} description={t("noServicesInViewDescription")} onRefresh={props.onRefresh} />
          )}
        </ScrollPane>
      </PanelCard>
    </div>
  )
}
