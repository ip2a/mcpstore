import { ArrowUpDownIcon, RefreshCwIcon } from "lucide-react"
import { useRef, useState, type ComponentRef } from "react"

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
import { ScrollPane } from "@/components/shared/scroll-pane"
import { CollapsibleSearchBox } from "@/components/shared/collapsible-search-box"
import { Button } from "@/components/ui/button"
import { ServiceList } from "@/features/services/service-list"
import { ServicesFilterDialog } from "@/features/services/services-filter-dialog"
import { type ServiceSortBy } from "@/features/services/use-services-list"
import { usePreserveServiceListScroll } from "@/features/services/use-preserve-service-list-scroll"
import { useServicesList } from "@/features/services/use-services-list"
import type { AgentItem, ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"

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
    applySort,
    filteredServices,
    moveService,
    query,
    scopeFilter,
    setAgentFilter,
    setQuery,
    setScopeFilter,
    setStatusFilter,
    statusFilter,
  } = useServicesList(props.services)
  const agentIds = props.agents.map((agent) => agent.agent_id)
  const listScrollRef = useRef<ComponentRef<typeof ScrollPane>>(null)
  const [sortOpen, setSortOpen] = useState(false)

  usePreserveServiceListScroll({
    busy: props.busy,
    listRootRef: listScrollRef,
    services: props.services,
  })

  const sortBy = (value: ServiceSortBy) => {
    applySort(value)
    setSortOpen(false)
  }

  return (
    <PanelCard className="h-full min-h-0 gap-0 p-0">
      <div className="flex w-full min-w-0 shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2">
        <label htmlFor="service-list-search" className="shrink-0 text-sm font-medium">
          {t("serviceList")}
        </label>
        <CollapsibleSearchBox id="service-list-search" placeholder={t("searchServices")} value={query} onChange={setQuery} />
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
          activeFilterCount={activeFilterCount}
          agentFilter={agentFilter}
          agentIds={agentIds}
          onAgentFilterChange={setAgentFilter}
          onScopeFilterChange={setScopeFilter}
          onStatusFilterChange={setStatusFilter}
          scopeFilter={scopeFilter}
          statusFilter={statusFilter}
        />
        <Button variant="outline" size="sm" onClick={props.onRefresh} disabled={props.loading}>
          <RefreshCwIcon data-icon="inline-start" />
          {t("refresh")}
        </Button>
      </div>
      <ScrollPane ref={listScrollRef} className="min-h-0 flex-1 px-4 py-2">
        {props.error ? (
          <PageError title={t("dashboardFailedToLoad")} message={props.error} onRefresh={props.onRefresh} />
        ) : props.loading && props.services.length === 0 ? (
          <PageSkeleton />
        ) : filteredServices.length ? (
          <ServiceList
            {...props}
            agents={props.agents}
            allServices={props.services}
            services={filteredServices}
            onDeclareScope={props.onDeclareScope}
            onMove={moveService}
          />
        ) : (
          <PageEmpty title={t("noServices")} description={t("noServicesInViewDescription")} onRefresh={props.onRefresh} />
        )}
      </ScrollPane>
    </PanelCard>
  )
}
