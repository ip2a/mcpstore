import { useEffect, useMemo, useState } from "react"
import { toast } from "sonner"

import { getAgentId } from "@/features/agents/model"
import { useAgentServicesQuery } from "@/features/agents/queries"
import { useInstanceToolsQueries } from "@/features/tools/queries"
import type { AgentItem, ServiceInstance } from "@/lib/api"
import { useUiStore } from "@/stores/ui-store"

export type SelectedScope =
  | { type: "store" }
  | { type: "agent"; agentId: string }

export function useAgentScope({
  agents,
  busy,
  selectedScope,
  services,
}: {
  agents: AgentItem[]
  busy: string | null
  selectedScope: SelectedScope
  services: ServiceInstance[]
}) {
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const selectedAgentId = useUiStore((state) => state.selectedAgentId)
  const setSelectedAgentId = useUiStore((state) => state.setSelectedAgentId)
  const agentId = selectedScope.type === "agent" ? selectedScope.agentId : ""
  const agentServicesQuery = useAgentServicesQuery(agentId)
  const scopeServices = useMemo(() => {
    if (selectedScope.type === "store") {
      return services.filter((service) => service.scope.type === "store")
    }
    return agentId ? agentServicesQuery.data || [] : []
  }, [agentId, agentServicesQuery.data, selectedScope.type, services])
  const storeServiceNames = useMemo(
    () => [...new Set(services.filter((service) => service.scope.type === "store").map((service) => service.service_name))],
    [services],
  )
  const addableServiceNames = useMemo(() => {
    if (selectedScope.type !== "agent") return []
    return storeServiceNames.filter(
      (name) => !scopeServices.some((service) => service.service_name === name),
    )
  }, [scopeServices, selectedScope.type, storeServiceNames])
  const [scopeServiceName, setScopeServiceName] = useState("")
  const scopeToolQueries = useInstanceToolsQueries(scopeServices, "available")
  const scopeTools = useMemo(
    () => scopeToolQueries.flatMap((result, index) =>
      (result.data || []).map((tool) => ({ instance: scopeServices[index], tool })),
    ),
    [scopeServices, scopeToolQueries],
  )
  const scopeServicesError = selectedScope.type === "agent" && agentId ? agentServicesQuery.error : null
  const scopeToolsError =
    selectedScope.type === "agent" && agentId
      ? scopeToolQueries.find((result) => result.error)?.error || null
      : null
  const scopeServicesErrorMessage =
    scopeServicesError instanceof Error
      ? scopeServicesError.message
      : scopeServicesError
        ? String(scopeServicesError)
        : "Agent services 加载失败"
  const scopeToolsErrorMessage =
    scopeToolsError instanceof Error
      ? scopeToolsError.message
      : scopeToolsError
        ? String(scopeToolsError)
        : "Agent tools 加载失败"
  const loadingScopeServices =
    selectedScope.type === "agent" && agentId ? agentServicesQuery.isFetching : false
  const loadingScopeTools = scopeToolQueries.some((result) => result.isFetching)

  useEffect(() => {
    if (!selectedAgentId && agentIds[0]) setSelectedAgentId(agentIds[0])
  }, [agentIds, selectedAgentId, setSelectedAgentId])

  useEffect(() => {
    if (!scopeServiceName || !addableServiceNames.includes(scopeServiceName)) {
      setScopeServiceName(addableServiceNames[0] || "")
    }
  }, [addableServiceNames, scopeServiceName])

  async function loadAgentScope() {
    if (selectedScope.type === "store") return
    if (!agentId) return
    const servicesResult = await agentServicesQuery.refetch()
    if (servicesResult.error) {
      toast.error(
        servicesResult.error instanceof Error ? servicesResult.error.message : "Agent services 加载失败",
      )
      return
    }
    const toolResults = await Promise.all(scopeToolQueries.map((result) => result.refetch()))
    const failed = toolResults.find((result) => result.error)
    if (failed?.error) toast.error(failed.error instanceof Error ? failed.error.message : "Agent tools 加载失败")
  }

  useEffect(() => {
    void loadAgentScope()
  }, [agentId, busy, selectedScope.type])

  return {
    activeAgentId: agentId,
    addableServiceNames,
    agentIds,
    loadAgentScope,
    loadingScopeServices,
    loadingScopeTools,
    scopeServiceName,
    scopeServices,
    scopeServicesError,
    scopeServicesErrorMessage,
    scopeTools,
    scopeToolsError,
    scopeToolsErrorMessage,
    setScopeServiceName,
    setSelectedAgentId,
  }
}
