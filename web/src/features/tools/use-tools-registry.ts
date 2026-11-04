import { useState } from "react"

import { useMcpRegistryEntries } from "@/features/mcp-registry/use-mcp-registry-entries"
import { useInstanceToolsQueries } from "@/features/tools/queries"
import {
  clearInstanceToolPolicy,
  listInstanceTools,
  setInstanceToolPolicy,
  type AgentItem,
  type ServiceInstance,
  type ToolInfo,
  type ToolVisibilityFilter,
} from "@/lib/api"

function countActiveToolFilters(filters: {
  scope: "store" | "agent"
  visibilityFilter: ToolVisibilityFilter
}) {
  let count = 0
  if (filters.scope !== "store") count++
  if (filters.visibilityFilter !== "available") count++
  return count
}

export function useToolsRegistry({
  agents,
  services,
}: {
  agents: AgentItem[]
  services: ServiceInstance[]
}) {
  const [visibilityFilter, setVisibilityFilter] = useState<ToolVisibilityFilter>("available")
  const { reload, visibleEntries, ...registry } = useMcpRegistryEntries<ToolInfo>({
    agents,
    services,
    // biome-ignore lint/correctness/useHookAtTopLevel: invoked unconditionally at the top of useMcpRegistryEntries during render
    useEntryQueries: (instances) => useInstanceToolsQueries(instances, visibilityFilter),
    matchQuery: matchesToolQuery,
    noun: "tools",
  })

  async function setToolAvailability(
    instance: ServiceInstance,
    tool: { name: string },
    available: boolean,
  ) {
    const current = await listInstanceTools(instance, "available")
    const names = current.map((candidate) => candidate.name).filter((name) => name !== tool.name)
    if (available) names.push(tool.name)
    await setInstanceToolPolicy(instance, names)
    await reload()
  }

  async function clearToolPolicy(instance: ServiceInstance) {
    await clearInstanceToolPolicy(instance)
    await reload()
  }

  const activeFilterCount = countActiveToolFilters({ scope: registry.scope, visibilityFilter })

  return {
    ...registry,
    activeFilterCount,
    clearToolPolicy,
    loadTools: reload,
    setToolAvailability,
    setVisibilityFilter,
    visibilityFilter,
    visibleTools: visibleEntries.map(({ instance, entry }) => ({ instance, tool: entry })),
  }
}

function matchesToolQuery(instance: ServiceInstance, tool: ToolInfo, query: string) {
  const needle = query.trim().toLowerCase()
  if (!needle) return true
  const scopeLabel = instance.scope.type === "store" ? "store" : instance.scope.agent_id
  const text =
    `${tool.name} ${tool.description} ${instance.service_name} ${scopeLabel}`.toLowerCase()
  return text.includes(needle)
}
