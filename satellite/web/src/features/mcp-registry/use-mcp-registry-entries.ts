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
  // Backend may skip individual instances (disconnected / auth pending); those
  // are expected and not surfaced as a page warning.
  // Agent services are only meaningful under agent scope; ignore their error /
  // fetching state while on store so a disabled-query force-refetch cannot
  // paint a Missing agent_id failure over a healthy store catalog.
  const agentScopeActive = scopeState.scope === "agent" && Boolean(scopeState.agentId)
  const queryError =
    entryQuery.error ?? (agentScopeActive ? scopeState.agentServicesQuery.error : null)
  const errorMessage = queryError
    ? queryError instanceof Error
      ? queryError.message
      : String(queryError)
    : `Failed to load ${noun}`
  const loading =
    entryQuery.isFetching || (agentScopeActive && scopeState.agentServicesQuery.isFetching)

  async function reload() {
    try {
      // refetch() ignores enabled:false — never force agent-scoped calls without an id.
      const results = await Promise.allSettled([
        ...(agentScopeActive ? [scopeState.agentServicesQuery.refetch()] : []),
        ...(agentScopeActive || scopeState.scope === "store" ? [entryQuery.refetch()] : []),
      ])
      const rejected = results.find(
        (result): result is PromiseRejectedResult => result.status === "rejected",
      )
      if (rejected) throw rejected.reason
    } catch (err) {
      toast.error(err instanceof Error ? err.message : `Failed to load ${noun}`)
    }
  }

  const selectedInstanceIds = useMemo(
    () => new Set(scopeState.selectedInstances.map((instance) => instance.instance_id)),
    [scopeState.selectedInstances],
  )
  const visibleEntries = entries.filter(
    ({ instance, entry }) =>
      selectedInstanceIds.has(instance.instance_id) &&
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
