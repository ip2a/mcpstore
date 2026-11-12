import { useCallback } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"

import { health, listAgents, listServices, type KvStoreName } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useDashboard() {
  const queryClient = useQueryClient()
  const healthQuery = useQuery({ queryKey: queryKeys.health, queryFn: health })
  const agentsQuery = useQuery({
    queryKey: queryKeys.agents,
    queryFn: () => listAgents().catch(() => []),
  })

  // Use all scope to fetch all services in a single request
  const servicesQuery = useQuery({
    queryKey: queryKeys.instances,
    queryFn: () => listServices({ type: "all" }),
    refetchInterval: (query) => {
      // Dynamic polling: faster when services are transitioning, slower when stable
      const services = query.state.data || []
      const hasTransitioningServices = services.some(
        (s) => s.state?.phase === "starting" || s.state?.phase === "stopping",
      )
      return hasTransitioningServices ? 3_000 : 12_000
    },
  })

  const services = servicesQuery.data || []
  const agents = agentsQuery.data || []

  // /health returns {status, store} with no store field; KV status comes from /kv/health (the KV page's own query)
  const kvStore: KvStoreName | undefined = undefined
  const loading = healthQuery.isLoading || servicesQuery.isLoading || agentsQuery.isLoading
  const errorSource = healthQuery.error || servicesQuery.error || agentsQuery.error
  const error =
    errorSource instanceof Error ? errorSource.message : errorSource ? String(errorSource) : null

  const refresh = useCallback(async () => {
    // exact: true — queryKeys.instances is a prefix of every per-instance
    // query (override/tools/…); a partial refetch would re-fan-out the grid.
    await Promise.all([
      queryClient.refetchQueries({ queryKey: queryKeys.health, exact: true }),
      queryClient.refetchQueries({ queryKey: queryKeys.instances, exact: true }),
      queryClient.refetchQueries({ queryKey: queryKeys.agents, exact: true }),
    ])
  }, [queryClient])

  return { services, agents, kvStore, loading, error, refresh }
}
