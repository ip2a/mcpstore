import { useEffect, useState } from "react"

import { getAgentId } from "@/features/agents/model"
import { useAgentServicesQuery } from "@/features/agents/queries"
import type { AgentItem, ServiceInstance } from "@/lib/api"

export function countActiveMcpRegistryFilters(filters: {
  instanceId: string
  scope: "store" | "agent"
}) {
  let count = 0
  if (filters.scope !== "store") count++
  if (filters.instanceId !== "all") count++
  return count
}

export function useMcpRegistryScope({ agents, services }: { agents: AgentItem[]; services: ServiceInstance[] }) {
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const [scope, setScope] = useState<"store" | "agent">("store")
  const [agentId, setAgentId] = useState(agentIds[0] || "")
  const [instanceId, setInstanceId] = useState("all")
  const [query, setQuery] = useState("")
  const agentServicesQuery = useAgentServicesQuery(scope === "agent" ? agentId : "")
  const scopeInstances = scope === "agent" ? agentServicesQuery.data || [] : services
  const selectedInstances =
    instanceId === "all"
      ? scopeInstances
      : scopeInstances.filter((instance) => instance.instance_id === instanceId)

  useEffect(() => {
    if (!agentId && agentIds[0]) setAgentId(agentIds[0])
  }, [agentId, agentIds])

  useEffect(() => {
    if (instanceId !== "all" && !scopeInstances.some((instance) => instance.instance_id === instanceId)) {
      setInstanceId("all")
    }
  }, [instanceId, scopeInstances])

  const activeFilterCount = countActiveMcpRegistryFilters({ instanceId, scope })

  return {
    activeFilterCount,
    agentId,
    agentIds,
    agentServicesQuery,
    instanceId,
    query,
    scope,
    scopeInstances,
    selectedInstances,
    setAgentId,
    setInstanceId,
    setQuery,
    setScope,
  }
}
