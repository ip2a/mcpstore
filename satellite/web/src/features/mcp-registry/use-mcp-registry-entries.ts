import { useMemo } from "react"
import { toast } from "sonner"
import type { UseQueryResult } from "@tanstack/react-query"

import { useMcpRegistryScope } from "@/features/mcp-registry/use-mcp-registry-scope"
import type { AgentItem, ServiceInstance } from "@/lib/api"

/**
 * Shared list engine for the tools/resources/prompts registries:
 * scope selection + per-instance queries + query filtering + reload.
 */
export function useMcpRegistryEntries<T>({
  agents,
  services,
  useEntryQueries,
  matchQuery,
  noun,
}: {
  agents: AgentItem[]
  services: ServiceInstance[]
  useEntryQueries: (instances: ServiceInstance[]) => UseQueryResult<T[], Error>[]
  matchQuery: (instance: ServiceInstance, entry: T, query: string) => boolean
  noun: string
}) {
  const scopeState = useMcpRegistryScope({ agents, services })
  const entryQueries = useEntryQueries(scopeState.selectedInstances)
  const entries = useMemo(
    () =>
      entryQueries.flatMap((result, index) =>
        (result.data || []).map((entry) => ({
          instance: scopeState.selectedInstances[index],
          entry,
        })),
      ),
    [entryQueries, scopeState.selectedInstances],
  )
  
  // Only show error if the service list query itself failed
  // Individual service errors (401, 500, etc.) are silently ignored
  const queryError = scopeState.agentServicesQuery.error
  
  const errorMessage =
    queryError instanceof Error
      ? queryError.message
      : queryError
        ? String(queryError)
        : `Failed to load ${noun}`
  const loading =
    scopeState.agentServicesQuery.isFetching || entryQueries.some((result) => result.isFetching)

  async function reload() {
    try {
      if (scopeState.scope === "agent") {
        const result = await scopeState.agentServicesQuery.refetch()
        if (result.error) throw result.error
      }
      // Refetch all entry queries, catching individual errors
      await Promise.allSettled(entryQueries.map((result) => result.refetch()))
    } catch (err) {
      toast.error(err instanceof Error ? err.message : `Failed to load ${noun}`)
    }
  }

  const visibleEntries = entries.filter(({ instance, entry }) =>
    matchQuery(instance, entry, scopeState.query),
  )

  return {
    ...scopeState,
    entries,
    error: queryError,
    errorMessage,
    loading,
    reload,
    visibleEntries,
  }
}
