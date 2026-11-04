import { useCallback } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"

import {
  health,
  listAgents,
  listServices,
  type CacheBackend,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useDashboard() {
  const queryClient = useQueryClient()
  const healthQuery = useQuery({ queryKey: queryKeys.health, queryFn: health })
  const agentsQuery = useQuery({ queryKey: queryKeys.agents, queryFn: () => listAgents().catch(() => []) })
  
  // Use root scope to fetch all services in a single request
  const servicesQuery = useQuery({
    queryKey: queryKeys.instances,
    queryFn: () => listServices({ type: "root" }),
    refetchInterval: (query) => {
      // Dynamic polling: faster when services are transitioning, slower when stable
      const services = query.state.data || []
      const hasTransitioningServices = services.some(
        (s) => s.state?.phase === "starting" || s.state?.phase === "stopping"
      )
      return hasTransitioningServices ? 3_000 : 12_000
    },
  })
  
  const services = servicesQuery.data || []
  const agents = agentsQuery.data || []
  
  // /health returns {status, store} with no backend field; cache status comes from /cache/health (the cache page's own query)
  const backend: CacheBackend | undefined = undefined
  const loading =
    healthQuery.isLoading ||
    servicesQuery.isLoading ||
    agentsQuery.isLoading
  const errorSource =
    healthQuery.error ||
    servicesQuery.error ||
    agentsQuery.error
  const error = errorSource instanceof Error ? errorSource.message : errorSource ? String(errorSource) : null

  const refresh = useCallback(async () => {
    // Use refetchQueries to reset the polling timer
    await Promise.all([
      queryClient.refetchQueries({ queryKey: queryKeys.health }),
      queryClient.refetchQueries({ queryKey: queryKeys.instances }),
      queryClient.refetchQueries({ queryKey: queryKeys.agents }),
    ])
  }, [queryClient])

  return { services, agents, backend, loading, error, refresh }
}
