import { useMcpRegistryEntries } from "@/features/mcp-registry/use-mcp-registry-entries"
import { useInstanceResourcesQueries } from "@/features/resources/queries"
import type { AgentItem, ResourceInfo, ServiceInstance } from "@/lib/api"

export function useResourcesRegistry({
  agents,
  services,
}: {
  agents: AgentItem[]
  services: ServiceInstance[]
}) {
  const { reload, visibleEntries, ...registry } = useMcpRegistryEntries<ResourceInfo>({
    agents,
    services,
    useEntryQueries: useInstanceResourcesQueries,
    matchQuery: matchesResourceQuery,
    noun: "resources",
  })
  return {
    ...registry,
    visibleResources: visibleEntries.map(({ instance, entry }) => ({ instance, resource: entry })),
    loadResources: reload,
  }
}

function matchesResourceQuery(instance: ServiceInstance, resource: ResourceInfo, query: string) {
  const needle = query.trim().toLowerCase()
  if (!needle) return true
  const scopeLabel = instance.scope.type === "store" ? "store" : instance.scope.agent_id
  const text =
    `${resource.name} ${resource.title || ""} ${resource.description || ""} ${resource.uri} ${instance.service_name} ${scopeLabel}`.toLowerCase()
  return text.includes(needle)
}
