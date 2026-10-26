import { useMemo } from "react"
import { toast } from "sonner"

import { useMcpRegistryScope } from "@/features/mcp-registry/use-mcp-registry-scope"
import { useInstanceResourcesQueries } from "@/features/resources/queries"
import type { AgentItem, ResourceInfo, ServiceInstance } from "@/lib/api"

export function useResourcesRegistry({ agents, services }: { agents: AgentItem[]; services: ServiceInstance[] }) {
  const scopeState = useMcpRegistryScope({ agents, services })
  const resourceQueries = useInstanceResourcesQueries(scopeState.selectedInstances)
  const resources = useMemo(
    () =>
      resourceQueries.flatMap((result, index) =>
        (result.data || []).map((resource) => ({
          instance: scopeState.selectedInstances[index],
          resource,
        })),
      ),
    [resourceQueries, scopeState.selectedInstances],
  )
  const queryError = scopeState.agentServicesQuery.error || resourceQueries.find((result) => result.error)?.error
  const errorMessage =
    queryError instanceof Error ? queryError.message : queryError ? String(queryError) : "Failed to load resources"
  const loading = scopeState.agentServicesQuery.isFetching || resourceQueries.some((result) => result.isFetching)

  async function loadResources() {
    try {
      if (scopeState.scope === "agent") {
        const result = await scopeState.agentServicesQuery.refetch()
        if (result.error) throw result.error
      }
      const results = await Promise.all(resourceQueries.map((result) => result.refetch()))
      const failed = results.find((result) => result.error)
      if (failed?.error) throw failed.error
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to load resources")
    }
  }

  const visibleResources = resources.filter(({ instance, resource }) => matchesResourceQuery(instance, resource, scopeState.query))

  return {
    ...scopeState,
    error: queryError,
    errorMessage,
    loadResources,
    loading,
    visibleResources,
  }
}

function matchesResourceQuery(instance: ServiceInstance, resource: ResourceInfo, query: string) {
  const needle = query.trim().toLowerCase()
  if (!needle) return true
  const scopeLabel = instance.scope.type === "store" ? "store" : instance.scope.agent_id
  const text = `${resource.name} ${resource.title || ""} ${resource.description || ""} ${resource.uri} ${instance.service_name} ${scopeLabel}`.toLowerCase()
  return text.includes(needle)
}
