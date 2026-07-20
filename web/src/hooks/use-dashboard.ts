import { useCallback } from "react"
import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query"

import { getAgentId } from "@/features/agents/model"
import {
  health,
  listAgentServices,
  listAgents,
  listServices,
  type CacheBackend,
  type ServiceInstance,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

function mergeScopedServices(storeServices: ServiceInstance[], agentGroups: ServiceInstance[][]) {
  const merged = new Map<string, ServiceInstance>()
  for (const service of storeServices) {
    merged.set(service.instance_id, service)
  }
  for (const group of agentGroups) {
    for (const service of group) {
      merged.set(service.instance_id, service)
    }
  }
  return Array.from(merged.values()).sort((a, b) => a.service_name.localeCompare(b.service_name))
}

export function useDashboard() {
  const queryClient = useQueryClient()
  const healthQuery = useQuery({ queryKey: queryKeys.health, queryFn: health })
  const agentsQuery = useQuery({ queryKey: queryKeys.agents, queryFn: () => listAgents().catch(() => []) })
  const storeServicesQuery = useQuery({
    queryKey: queryKeys.instances,
    queryFn: () => listServices(),
    refetchInterval: 5_000,
  })
  const agentIds = (agentsQuery.data || []).map(getAgentId).filter(Boolean)
  const agentServicesQueries = useQueries({
    queries: agentIds.map((agentId) => ({
      queryKey: queryKeys.agentServices(agentId),
      queryFn: () => listAgentServices(agentId),
      enabled: agentsQuery.isSuccess,
      refetchInterval: 5_000,
    })),
  })
  const services = mergeScopedServices(
    storeServicesQuery.data || [],
    agentServicesQueries.map((query) => query.data || []),
  )
  const agents = agentsQuery.data || []
  const backend = healthQuery.data?.backend as CacheBackend | undefined
  const loading =
    healthQuery.isLoading ||
    storeServicesQuery.isLoading ||
    agentsQuery.isLoading ||
    agentServicesQueries.some((query) => query.isLoading)
  const errorSource =
    healthQuery.error ||
    storeServicesQuery.error ||
    agentsQuery.error ||
    agentServicesQueries.find((query) => query.error)?.error
  const error = errorSource instanceof Error ? errorSource.message : errorSource ? String(errorSource) : null

  const refresh = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.health }),
      queryClient.invalidateQueries({ queryKey: queryKeys.instances }),
      queryClient.invalidateQueries({ queryKey: queryKeys.agents }),
    ])
  }, [queryClient])

  return { services, agents, backend, loading, error, refresh }
}
