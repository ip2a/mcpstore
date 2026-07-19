import { useEffect, useMemo, useState } from "react"
import { toast } from "sonner"

import { getAgentId } from "@/features/agents/model"
import { useAgentServicesQuery } from "@/features/agents/queries"
import { type InstanceTool, useInstanceToolsQueries } from "@/features/tools/queries"
import type { ToolDetailState, ToolDialogState } from "@/features/tools/tool-dialogs"
import {
  callInstanceTool,
  clearInstanceToolPolicy,
  listInstanceTools,
  setInstanceToolPolicy,
  type AgentItem,
  type ServiceInstance,
  type ToolVisibilityFilter,
} from "@/lib/api"
import { toolKey } from "@/lib/tool-info"

export function useToolsRegistry({ agents, services }: { agents: AgentItem[]; services: ServiceInstance[] }) {
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const [scope, setScope] = useState<"store" | "agent">("store")
  const [agentId, setAgentId] = useState(agentIds[0] || "")
  const [instanceId, setInstanceId] = useState("all")
  const [query, setQuery] = useState("")
  const [visibilityFilter, setVisibilityFilter] = useState<ToolVisibilityFilter>("available")
  const [selectedToolKey, setSelectedToolKey] = useState<string | null>(null)
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
  const errorMessage = queryError instanceof Error ? queryError.message : queryError ? String(queryError) : "工具加载失败"
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
    const current = await listInstanceTools(instance.instance_id, "available")
    const names = current
      .map((candidate) => candidate.name)
      .filter((name) => name !== tool.name)
    if (available) names.push(tool.name)
    await setInstanceToolPolicy(instance.instance_id, names)
    await loadTools()
  }

  async function clearToolPolicy(instance: ServiceInstance) {
    await clearInstanceToolPolicy(instance.instance_id)
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
      toast.error(err instanceof Error ? err.message : "工具加载失败")
    }
  }

  const visibleTools = tools.filter(({ instance, tool }) => {
    const scopeLabel = instance.scope.type === "store" ? "store" : instance.scope.agent_id
    const text = `${tool.name} ${tool.description} ${instance.service_name} ${scopeLabel}`.toLowerCase()
    return text.includes(query.trim().toLowerCase())
  })
  const selectedTool = visibleTools.find(({ instance, tool }) => toolKey(instance.instance_id, tool) === selectedToolKey)
    || visibleTools[0]

  useEffect(() => {
    setSelectedToolKey(selectedTool ? toolKey(selectedTool.instance.instance_id, selectedTool.tool) : null)
  }, [selectedTool])

  function makeRunner(item: InstanceTool): NonNullable<ToolDialogState> & NonNullable<ToolDetailState> {
    const { instance, tool } = item
    const scopeLabel = instance.scope.type === "store" ? "store" : `agent ${instance.scope.agent_id}`
    return {
      tool,
      service: instance,
      sourceLabel: `${instance.service_name} · ${scopeLabel}`,
      onRun: (args) => callInstanceTool(instance.instance_id, tool.name, args),
    }
  }

  return {
    agentId,
    agentIds,
    error: queryError,
    errorMessage,
    instanceId,
    loadTools,
    loading,
    makeRunner,
    query,
    scope,
    scopeInstances,
    selectedTool,
    selectedToolKey,
    setAgentId,
    setInstanceId,
    setToolAvailability,
    clearToolPolicy,
    setVisibilityFilter,
    visibilityFilter,
    setQuery,
    setScope,
    setSelectedToolKey,
    visibleTools,
  }
}
