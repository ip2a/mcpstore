import { ArrowUpDownIcon, RefreshCwIcon } from "lucide-react"
import { useRef, type ComponentRef } from "react"

import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Field, FieldLabel } from "@/components/ui/field"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
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
  } = useServicesList(props.services)
  const agentIds = props.agents.map((agent) => agent.agent_id)
  const listScrollRef = useRef<ComponentRef<typeof ScrollPane>>(null)

  usePreserveServiceListScroll({
    busy: props.busy,
    listRootRef: listScrollRef,
    services: props.services,
  })

  return (
    <PanelCard className="h-full min-h-0 gap-0 p-0">
      <div className="flex w-full min-w-0 shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2">
        <label htmlFor="service-list-search" className="shrink-0 text-sm font-medium">
          {t("serviceList")}
        </label>
        <CollapsibleSearchBox id="service-list-search" placeholder={t("searchServices")} value={query} onChange={setQuery} />
        <Dialog>
          <DialogTrigger asChild>
            <Button variant="outline" size="sm">
              <ArrowUpDownIcon data-icon="inline-start" />
              {t("sortLabel")}
            </Button>
          </DialogTrigger>
          <DialogContent className="sm:max-w-sm">
            <DialogHeader>
              <DialogTitle>{t("sortLabel")}</DialogTitle>
            </DialogHeader>
            <Field>
              <FieldLabel>{t("sortLabel")}</FieldLabel>
              <Select value={sortBy} onValueChange={(value) => setSortBy(value as ServiceSortBy)}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder={t("sortLabel")} />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="status">{t("sortStatus")}</SelectItem>
                    <SelectItem value="name">{t("sortName")}</SelectItem>
                    <SelectItem value="tools">{t("sortTools")}</SelectItem>
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
            <DialogFooter showCloseButton />
          </DialogContent>
        </Dialog>
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
          />
        ) : (
          <PageEmpty title={t("noServices")} description={t("noServicesInViewDescription")} onRefresh={props.onRefresh} />
        )}
      </ScrollPane>
    </PanelCard>
  )
}
