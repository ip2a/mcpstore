import { useEffect, useMemo, useState } from "react"
import { toast } from "sonner"

import { getAgentId } from "@/features/agents/model"
import { useAgentServicesQuery } from "@/features/agents/queries"
import { useInstanceToolsQueries } from "@/features/tools/queries"
import type { AgentItem, ServiceInstance } from "@/lib/api"
import { useUiStore } from "@/stores/ui-store"

export function useAgentScope({ agents, busy, services }: { agents: AgentItem[]; busy: string | null; services: ServiceInstance[] }) {
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const selectedAgentId = useUiStore((state) => state.selectedAgentId)
  const setSelectedAgentId = useUiStore((state) => state.setSelectedAgentId)
  const [typedAgentId, setTypedAgentId] = useState("")
  const serviceNames = useMemo(() => [...new Set(services.map((service) => service.service_name))], [services])
  const [scopeServiceName, setScopeServiceName] = useState(serviceNames[0] || "")
  const activeAgentId = (typedAgentId.trim() || selectedAgentId || "").trim()
  const agentServicesQuery = useAgentServicesQuery(activeAgentId)
  const agentServices = activeAgentId ? agentServicesQuery.data || [] : []
  const agentToolQueries = useInstanceToolsQueries(agentServices, "available")
  const agentTools = useMemo(
    () => agentToolQueries.flatMap((result, index) =>
      (result.data || []).map((tool) => ({ instance: agentServices[index], tool })),
    ),
    [agentServices, agentToolQueries],
  )
  const agentServicesError = activeAgentId ? agentServicesQuery.error : null
  const agentToolsError = activeAgentId ? agentToolQueries.find((result) => result.error)?.error || null : null
  const agentServicesErrorMessage = agentServicesError instanceof Error ? agentServicesError.message : agentServicesError ? String(agentServicesError) : "Agent services 加载失败"
  const agentToolsErrorMessage = agentToolsError instanceof Error ? agentToolsError.message : agentToolsError ? String(agentToolsError) : "Agent tools 加载失败"
  const loadingAgentServices = agentServicesQuery.isFetching
  const loadingAgentTools = agentToolQueries.some((result) => result.isFetching)

  useEffect(() => {
    if (!selectedAgentId && agentIds[0]) setSelectedAgentId(agentIds[0])
  }, [agentIds, selectedAgentId, setSelectedAgentId])

  useEffect(() => {
    if (!scopeServiceName && serviceNames[0]) setScopeServiceName(serviceNames[0])
  }, [scopeServiceName, serviceNames])

  async function loadAgentScope() {
    if (!activeAgentId) return
    const servicesResult = await agentServicesQuery.refetch()
    if (servicesResult.error) {
      toast.error(servicesResult.error instanceof Error ? servicesResult.error.message : "Agent services 加载失败")
      return
    }
    const toolResults = await Promise.all(agentToolQueries.map((result) => result.refetch()))
    const failed = toolResults.find((result) => result.error)
    if (failed?.error) toast.error(failed.error instanceof Error ? failed.error.message : "Agent tools 加载失败")
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
    loadAgentScope,
    loadingAgentServices,
    loadingAgentTools,
    scopeServiceName,
    selectedAgentId,
    serviceNames,
    setScopeServiceName,
    setSelectedAgentId,
    setTypedAgentId,
    typedAgentId,
  }
}
