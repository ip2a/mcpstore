import { useEffect, useState } from "react"

import { getAgentId } from "@/features/agents/model"
import { useAgentServicesQuery } from "@/features/agents/queries"
import type { AgentItem, ServiceInstance } from "@/lib/api"

export function countActiveMcpRegistryFilters(filters: { scope: "store" | "agent" }) {
  let count = 0
  if (filters.scope !== "store") count++
  return count
}

export function useMcpRegistryScope({
  agents,
  services,
}: {
  agents: AgentItem[]
  services: ServiceInstance[]
}) {
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const [scope, setScope] = useState<"store" | "agent">("store")
  const [agentId, setAgentId] = useState(agentIds[0] || "")
  const [instanceId, setInstanceId] = useState("all")
  const [query, setQuery] = useState("")
  const agentServicesQuery = useAgentServicesQuery(scope === "agent" ? agentId : "")
  
  // Sort instances alphabetically by service_name for stable ordering
  const scopeInstances = (scope === "agent" ? agentServicesQuery.data || [] : services)
    .slice()
    .sort((a, b) => a.service_name.localeCompare(b.service_name))
  
  const selectedInstances =
    instanceId === "all"
      ? scopeInstances
      : scopeInstances.filter((instance) => instance.instance_id === instanceId)

  useEffect(() => {
    if (!agentId && agentIds[0]) setAgentId(agentIds[0])
  }, [agentId, agentIds])

  useEffect(() => {
    setInstanceId("all")
  }, [scope, agentId])

  useEffect(() => {
    if (
      instanceId !== "all" &&
      !scopeInstances.some((instance) => instance.instance_id === instanceId)
    ) {
      setInstanceId("all")
    }
  }, [instanceId, scopeInstances])

  const activeFilterCount = countActiveMcpRegistryFilters({ scope })

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
