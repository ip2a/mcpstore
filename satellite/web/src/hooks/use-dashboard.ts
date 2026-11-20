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

  // Shared snapshot for routing / non-status pages. No background polling —
  // Services / Agents / Scope / detail own live refresh while mounted.
  const servicesQuery = useQuery({
    queryKey: queryKeys.instances,
    queryFn: () => listServices({ type: "all" }),
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
