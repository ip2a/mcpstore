import { ActivityIcon, DatabaseIcon } from "lucide-react"

import { HomeHero } from "@/components/home-hero"
import { ActivitySparkline } from "@/components/shared/activity-sparkline"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SearchBox } from "@/components/shared/search-box"
import { SectionHeading } from "@/components/shared/section-heading"
import { Button } from "@/components/ui/button"
import { ServiceList } from "@/features/services/service-list"
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
  const { filteredServices, query, setQuery, totals } = useServicesList(props.services)
  const agentIds = props.agents.map((agent) => agent.agent_id)

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3 overflow-hidden">
      <HomeHero
        backend={props.backend}
        stats={{
          loading: props.loading,
          services: totals.services,
          connecting: totals.connecting,
          agents: agentIds.length,
        }}
      />

      <PanelCard className="min-h-0">
        <SectionHeading
          title={t("serviceList")}
          titleAs="h2"
          titleAddon={
            <ActivitySparkline
              className="min-w-[120px] max-w-[280px] flex-1"
              values={[totals.services, totals.connecting, agentIds.length]}
              isLoading={props.loading}
              title={t("storeActivityOverview")}
            />
          }
          className="shrink-0 border-b-0 pb-0 @min-[42rem]:grid-cols-[auto_minmax(0,1fr)] @min-[42rem]:items-center"
          actions={
            <>
              <div className="min-w-0 @min-[28rem]:col-span-2 @min-[42rem]:col-span-1">
                <SearchBox placeholder={t("searchServices")} value={query} onChange={setQuery} />
              </div>
              <Button variant="outline" onClick={props.onCache}>
                <DatabaseIcon data-icon="inline-start" />
                {t("cache")}
              </Button>
              <Button variant="outline" onClick={props.onCheck} disabled={Boolean(props.busy)}>
                <ActivityIcon data-icon="inline-start" />
                {t("inspect")}
              </Button>
            </>
          }
          actionsProps={{
            className:
                "grid w-full min-w-0 grid-cols-1 items-center gap-2 @min-[28rem]:grid-cols-2 @min-[42rem]:grid-cols-[minmax(0,1fr)_auto_auto] @min-[42rem]:justify-end",
          }}
        />
        <ScrollPane className="min-h-0 flex-1">
          {props.error ? (
            <PageError title={t("dashboardFailedToLoad")} message={props.error} onRefresh={props.onRefresh} />
          ) : props.loading ? (
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
