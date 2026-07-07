import { useEffect, useState } from "react"
import { toast } from "sonner"

import { getAgentId } from "@/features/agents/model"
import { useToolsQuery } from "@/features/tools/queries"
import type { ToolDialogState } from "@/features/tools/tool-dialogs"
import { callAgentTool, callStoreTool, type AgentItem, type ToolInfo } from "@/lib/api"
import { getToolServiceName, toolKey } from "@/lib/tool-info"

export function useToolsRegistry({ agents }: { agents: AgentItem[] }) {
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const [scope, setScope] = useState("store")
  const [agentId, setAgentId] = useState(agentIds[0] || "")
  const [serviceName, setServiceName] = useState("all")
  const [query, setQuery] = useState("")
  const [selectedToolKey, setSelectedToolKey] = useState<string | null>(null)
  const serviceFilter = serviceName === "all" ? undefined : serviceName
  const toolsQuery = useToolsQuery({ agentId, scope, serviceName: serviceFilter })
  const tools = toolsQuery.data || []
  const error = toolsQuery.error
  const errorMessage = error instanceof Error ? error.message : error ? String(error) : "工具加载失败"
  const loading = toolsQuery.isFetching

  useEffect(() => {
    if (!agentId && agentIds[0]) setAgentId(agentIds[0])
  }, [agentId, agentIds])

  async function loadTools() {
    try {
      const nextTools = await toolsQuery.refetch()
      if (nextTools.error) throw nextTools.error
    } catch (err) {
      const message = err instanceof Error ? err.message : "工具加载失败"
      toast.error(message)
    }
  }

  useEffect(() => {
    void loadTools()
  }, [scope, agentId, serviceName])

  const visibleTools = tools.filter((tool) => {
    const text = `${tool.name} ${tool.description || ""} ${getToolServiceName(tool) || ""}`.toLowerCase()
    return text.includes(query.trim().toLowerCase())
  })
  const selectedTool = visibleTools.find((tool) => toolKey(tool) === selectedToolKey) || visibleTools[0]

  useEffect(() => {
    setSelectedToolKey(selectedTool ? toolKey(selectedTool) : null)
  }, [selectedTool])

  function makeRunner(tool: ToolInfo): NonNullable<ToolDialogState> {
    const sourceLabel = scope === "agent" ? `Agent ${agentId}` : getToolServiceName(tool) || serviceName
    return {
      tool,
      sourceLabel,
      onRun: (args) => (scope === "agent" ? callAgentTool(agentId, tool.name, args) : callStoreTool(tool.name, args)),
    }
  }

  return {
    agentId,
    agentIds,
    error,
    errorMessage,
    loadTools,
    loading,
    makeRunner,
    query,
    scope,
    selectedTool,
    selectedToolKey,
    serviceName,
    setAgentId,
    setQuery,
    setScope,
    setSelectedToolKey,
    setServiceName,
    visibleTools,
  }
}
