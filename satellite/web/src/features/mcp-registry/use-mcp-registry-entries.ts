import { useMemo } from "react"
import { toast } from "sonner"
import type { UseQueryResult } from "@tanstack/react-query"

import { useMcpRegistryScope } from "@/features/mcp-registry/use-mcp-registry-scope"
import type { AgentItem, ScopeCatalogPage, ServiceInstance } from "@/lib/api"

/**
 * Shared list engine for the tools/resources/prompts registries: scope
 * selection + ONE scope-catalog query (backend aggregation; entries carry
 * their instance identity) + query filtering + reload. Per-instance tabs are
 * filtered client-side — switching tabs never refetches.
 */
export function useMcpRegistryEntries<T extends { instance_id: string }>({
  agents,
  services,
  useScopeQuery,
  matchQuery,
  noun,
}: {
  agents: AgentItem[]
  services: ServiceInstance[]
  useScopeQuery: (
    scope: "store" | "agent",
    agentId: string,
  ) => UseQueryResult<ScopeCatalogPage<T>, Error>
  matchQuery: (instance: ServiceInstance, entry: T, query: string) => boolean
  noun: string
}) {
  const scopeState = useMcpRegistryScope({ agents, services })
  const entryQuery = useScopeQuery(scopeState.scope, scopeState.agentId)
  const instanceById = useMemo(
    () => new Map(scopeState.scopeInstances.map((instance) => [instance.instance_id, instance])),
    [scopeState.scopeInstances],
  )
  const entries = useMemo(
    () =>
      (entryQuery.data?.entries || []).flatMap((entry) => {
        const instance = instanceById.get(entry.instance_id)
        return instance ? [{ instance, entry }] : []
      }),
    [entryQuery.data, instanceById],
  )

  // Only show error if the catalog query or the service list itself failed.
  // Instances the backend skipped surface as a count (their listings can fail
  // for good reasons — disconnected service, OAuth pending) so a partial
  // catalog is at least visible.
  const queryError = entryQuery.error ?? scopeState.agentServicesQuery.error
  const failedInstanceCount = entryQuery.data?.skippedInstances ?? 0
  const errorMessage = queryError
    ? queryError instanceof Error
      ? queryError.message
      : String(queryError)
    : `Failed to load ${noun}`
  const loading = entryQuery.isFetching || scopeState.agentServicesQuery.isFetching

  async function reload() {
    try {
      const results = await Promise.allSettled([
        scopeState.agentServicesQuery.refetch(),
        entryQuery.refetch(),
      ])
      const rejected = results.find(
        (result): result is PromiseRejectedResult => result.status === "rejected",
      )
      if (rejected) throw rejected.reason
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
    failedInstanceCount,
    loading,
    reload,
    visibleEntries,
  }
}
