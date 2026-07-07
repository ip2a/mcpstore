import { useEffect, useState } from "react"
import { toast } from "sonner"

import { getAgentId } from "@/features/agents/model"
import { useAgentServicesQuery, useAgentToolsQuery } from "@/features/agents/queries"
import type { AgentItem, ServiceEntry } from "@/lib/api"
import { useUiStore } from "@/stores/ui-store"

export function useAgentScope({ agents, busy, services }: { agents: AgentItem[]; busy: string | null; services: ServiceEntry[] }) {
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const selectedAgentId = useUiStore((state) => state.selectedAgentId)
  const setSelectedAgentId = useUiStore((state) => state.setSelectedAgentId)
  const [typedAgentId, setTypedAgentId] = useState("")
  const [assignTarget, setAssignTarget] = useState(services[0]?.name || "")
  const activeAgentId = (typedAgentId.trim() || selectedAgentId || "").trim()
  const agentServicesQuery = useAgentServicesQuery(activeAgentId)
  const agentToolsQuery = useAgentToolsQuery(activeAgentId)
  const agentServices = activeAgentId ? agentServicesQuery.data || [] : []
  const agentTools = activeAgentId ? agentToolsQuery.data || [] : []
  const agentServicesError = activeAgentId ? agentServicesQuery.error : null
  const agentToolsError = activeAgentId ? agentToolsQuery.error : null
  const agentServicesErrorMessage = agentServicesError instanceof Error ? agentServicesError.message : agentServicesError ? String(agentServicesError) : "Agent services 加载失败"
  const agentToolsErrorMessage = agentToolsError instanceof Error ? agentToolsError.message : agentToolsError ? String(agentToolsError) : "Agent tools 加载失败"
  const loadingAgentServices = agentServicesQuery.isFetching
  const loadingAgentTools = agentToolsQuery.isFetching

  useEffect(() => {
    if (!selectedAgentId && agentIds[0]) setSelectedAgentId(agentIds[0])
  }, [agentIds, selectedAgentId, setSelectedAgentId])

  useEffect(() => {
    if (!assignTarget && services[0]?.name) setAssignTarget(services[0].name)
  }, [assignTarget, services])

  async function loadAgentScope() {
    if (!activeAgentId) return
    const [servicesResult, toolsResult] = await Promise.all([agentServicesQuery.refetch(), agentToolsQuery.refetch()])
    if (servicesResult.error) toast.error(servicesResult.error instanceof Error ? servicesResult.error.message : "Agent services 加载失败")
    if (toolsResult.error) toast.error(toolsResult.error instanceof Error ? toolsResult.error.message : "Agent tools 加载失败")
  }

  useEffect(() => {
    void loadAgentScope()
  }, [activeAgentId, busy])

  return {
    activeAgentId,
    agentIds,
    agentServices,
    agentServicesError,
    agentServicesErrorMessage,
    agentTools,
    agentToolsError,
    agentToolsErrorMessage,
    assignTarget,
    loadAgentScope,
    loadingAgentServices,
    loadingAgentTools,
    selectedAgentId,
    setAssignTarget,
    setSelectedAgentId,
    setTypedAgentId,
    typedAgentId,
  }
}
