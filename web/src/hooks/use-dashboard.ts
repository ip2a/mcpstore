import { useCallback } from "react"
import { useQuery } from "@tanstack/react-query"

import { health, listAgents, listServices, type AgentItem, type CacheBackend, type ServiceInstance } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useDashboard() {
  const healthQuery = useQuery({ queryKey: queryKeys.health, queryFn: health })
  const servicesQuery = useQuery({
    queryKey: queryKeys.instances,
    queryFn: async () => (await listServices()).sort((a, b) => a.service_name.localeCompare(b.service_name)),
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

  return { services, agents, backend, loading, error, refresh }
}
