import { useEffect, useMemo, useState } from "react"
import { toast } from "sonner"

import { getAgentId } from "@/features/agents/model"
import { useAgentServicesQuery } from "@/features/agents/queries"
import { useInstanceToolsQueries } from "@/features/tools/queries"
import {
  clearInstanceToolPolicy,
  listInstanceTools,
  setInstanceToolPolicy,
  type AgentItem,
  type ServiceInstance,
  type ToolVisibilityFilter,
} from "@/lib/api"

export function countActiveToolFilters(filters: {
  instanceId: string
  scope: "store" | "agent"
  visibilityFilter: ToolVisibilityFilter
}) {
  let count = 0
  if (filters.scope !== "store") count++
  if (filters.instanceId !== "all") count++
  if (filters.visibilityFilter !== "available") count++
  return count
}

export function useToolsRegistry({ agents, services }: { agents: AgentItem[]; services: ServiceInstance[] }) {
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const [scope, setScope] = useState<"store" | "agent">("store")
  const [agentId, setAgentId] = useState(agentIds[0] || "")
  const [instanceId, setInstanceId] = useState("all")
  const [query, setQuery] = useState("")
  const [visibilityFilter, setVisibilityFilter] = useState<ToolVisibilityFilter>("available")
  const agentServicesQuery = useAgentServicesQuery(scope === "agent" ? agentId : "")
  const scopeInstances = scope === "agent" ? agentServicesQuery.data || [] : services
  const selectedInstances = instanceId === "all"
    ? scopeInstances
    : scopeInstances.filter((instance) => instance.instance_id === instanceId)
  const toolQueries = useInstanceToolsQueries(selectedInstances, visibilityFilter)
  const tools = useMemo(
    () => toolQueries.flatMap((result, index) =>
      (result.data || []).map((tool) => ({ instance: selectedInstances[index], tool })),
    ),
    [selectedInstances, toolQueries],
  )
  const queryError = agentServicesQuery.error || toolQueries.find((result) => result.error)?.error
  const errorMessage = queryError instanceof Error ? queryError.message : queryError ? String(queryError) : "Failed to load tools"
  const loading = agentServicesQuery.isFetching || toolQueries.some((result) => result.isFetching)

  useEffect(() => {
    if (!agentId && agentIds[0]) setAgentId(agentIds[0])
  }, [agentId, agentIds])

  useEffect(() => {
    if (instanceId !== "all" && !scopeInstances.some((instance) => instance.instance_id === instanceId)) {
      setInstanceId("all")
    }
  }, [instanceId, scopeInstances])

  async function setToolAvailability(instance: ServiceInstance, tool: { name: string }, available: boolean) {
    const current = await listInstanceTools(instance, "available")
    const names = current
      .map((candidate) => candidate.name)
      .filter((name) => name !== tool.name)
    if (available) names.push(tool.name)
    await setInstanceToolPolicy(instance, names)
    await loadTools()
  }

  async function clearToolPolicy(instance: ServiceInstance) {
    await clearInstanceToolPolicy(instance)
    await loadTools()
  }

  async function loadTools() {
    try {
      if (scope === "agent") {
        const result = await agentServicesQuery.refetch()
        if (result.error) throw result.error
      }
      const results = await Promise.all(toolQueries.map((result) => result.refetch()))
      const failed = results.find((result) => result.error)
      if (failed?.error) throw failed.error
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to load tools")
    }
  }

  const visibleTools = tools.filter(({ instance, tool }) => {
    const scopeLabel = instance.scope.type === "store" ? "store" : instance.scope.agent_id
    const text = `${tool.name} ${tool.description} ${instance.service_name} ${scopeLabel}`.toLowerCase()
    return text.includes(query.trim().toLowerCase())
  })
  const activeFilterCount = countActiveToolFilters({ instanceId, scope, visibilityFilter })

  return {
    activeFilterCount,
    agentId,
    agentIds,
    error: queryError,
    errorMessage,
    instanceId,
    loadTools,
    loading,
    query,
    scope,
    scopeInstances,
    setAgentId,
    setInstanceId,
    setToolAvailability,
    clearToolPolicy,
    setVisibilityFilter,
    visibilityFilter,
    setQuery,
    setScope,
    visibleTools,
  }
}
