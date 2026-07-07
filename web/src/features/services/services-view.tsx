import { ActivityIcon, DatabaseIcon, RefreshCwIcon } from "lucide-react"

import { HomeHero } from "@/components/home-hero"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SearchBox } from "@/components/shared/search-box"
import { SectionHeading } from "@/components/shared/section-heading"
import { Button } from "@/components/ui/button"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { ServiceTable } from "@/features/services/service-table"
import { useServicesList } from "@/features/services/use-services-list"
import type { AgentItem, CacheBackend, ServiceEntry } from "@/lib/api"

export function ServicesView(props: {
  services: ServiceEntry[]
  agents: AgentItem[]
  agentMap: Map<string, string>
  backend?: CacheBackend
  busy: string | null
  error: string | null
  loading: boolean
  onCache: () => void
  onCheck: () => void
  onConnect: (service: ServiceEntry) => void
  onDelete: (service: ServiceEntry) => void
  onDisconnect: (service: ServiceEntry) => void
  onOpen: (service: ServiceEntry) => void
  onRefresh: () => void
  onRestart: (service: ServiceEntry) => void
}) {
  const { agentFilter, agentIds, filteredServices, query, setAgentFilter, setQuery, totals } = useServicesList({
    agents: props.agents,
    agentMap: props.agentMap,
    services: props.services,
  })

  return (
    <>
      <HomeHero
        backend={props.backend}
        stats={{
          loading: props.loading,
          services: totals.services,
          connected: totals.connected,
          disconnected: totals.disconnected,
          connecting: totals.connecting,
          error: totals.error,
          tools: totals.tools,
          agents: agentIds.length,
        }}
      />

      <PanelCard>
        <SectionHeading
          title="MCP 服务列表"
          titleAs="h2"
          description={`${filteredServices.length} services`}
          className="border-b-0 pb-0"
          actions={
            <Button variant="outline" size="sm" onClick={props.onCache}>
              <DatabaseIcon data-icon="inline-start" />
              缓存
            </Button>
          }
        />
        <div className="flex flex-col gap-4">
          <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_220px_auto_auto]">
            <SearchBox placeholder="Search services" value={query} onChange={setQuery} />
            <Select value={agentFilter} onValueChange={setAgentFilter}>
              <SelectTrigger>
                <SelectValue placeholder="Agent" />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="store">Store</SelectItem>
                  {agentIds.map((agentId) => (
                    <SelectItem key={agentId} value={agentId}>
                      {agentId}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
            <Button variant="outline" onClick={props.onRefresh} disabled={props.loading}>
              <RefreshCwIcon data-icon="inline-start" />
              刷新
            </Button>
            <Button variant="outline" onClick={props.onCheck} disabled={Boolean(props.busy)}>
              <ActivityIcon data-icon="inline-start" />
              检查
            </Button>
          </div>
          {props.error ? (
            <PageError title="Dashboard failed to load" message={props.error} onRefresh={props.onRefresh} />
          ) : props.loading ? (
            <PageSkeleton />
          ) : filteredServices.length ? (
            <ServiceTable {...props} services={filteredServices} />
          ) : (
            <PageEmpty title="No services" description="No MCP services are available in the current view." onRefresh={props.onRefresh} />
          )}
        </div>
      </PanelCard>
    </>
  )
}
