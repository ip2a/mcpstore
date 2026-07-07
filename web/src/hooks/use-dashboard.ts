import { useCallback, useMemo } from "react"
import { useQuery } from "@tanstack/react-query"

import { health, listAgents, listServices, type AgentItem, type CacheBackend, type ServiceEntry } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useDashboard() {
  const healthQuery = useQuery({ queryKey: queryKeys.health, queryFn: health })
  const servicesQuery = useQuery({
    queryKey: queryKeys.services,
    queryFn: async () => (await listServices()).sort((a, b) => a.name.localeCompare(b.name)),
  })
  const agentsQuery = useQuery({ queryKey: queryKeys.agents, queryFn: () => listAgents().catch(() => []) })

  const services = servicesQuery.data || []
  const agents = agentsQuery.data || []
  const backend = healthQuery.data?.backend as CacheBackend | undefined
  const loading = healthQuery.isFetching || servicesQuery.isFetching || agentsQuery.isFetching
  const errorSource = healthQuery.error || servicesQuery.error
  const error = errorSource instanceof Error ? errorSource.message : errorSource ? String(errorSource) : null
  const refetchHealth = healthQuery.refetch
  const refetchServices = servicesQuery.refetch
  const refetchAgents = agentsQuery.refetch

  const refresh = useCallback(async () => {
    await Promise.all([refetchHealth(), refetchServices(), refetchAgents()])
  }, [refetchAgents, refetchHealth, refetchServices])

  const agentMap = useMemo(() => {
    const map = new Map<string, string>()
    for (const agent of agents) {
      const agentId = String(agent.agent_id || agent.id || "")
      const serviceNames = agent.services || agent.service_names || []
      for (const name of serviceNames) {
        map.set(String(name), agentId)
      }
    }
    return map
  }, [agents])

  return { services, agents, agentMap, backend, loading, error, refresh }
}
