import { useMemo, useState } from "react"

import { getAgentId } from "@/features/agents/model"
import type { AgentItem, ServiceEntry } from "@/lib/api"

export function useServicesList({ agents, agentMap, services }: { agents: AgentItem[]; agentMap: Map<string, string>; services: ServiceEntry[] }) {
  const [agentFilter, setAgentFilter] = useState("store")
  const [query, setQuery] = useState("")
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const filteredServices = useMemo(() => {
    return services.filter((service) => {
      const inAgent = agentFilter === "store" || agentMap.get(service.name) === agentFilter
      const text = `${service.name} ${service.transport || ""} ${service.config?.description || ""}`.toLowerCase()
      return inAgent && text.includes(query.trim().toLowerCase())
    })
  }, [agentFilter, agentMap, services, query])
  const totals = useMemo(() => {
    const count = (status: string) => filteredServices.filter((service) => service.status === status).length
    return {
      services: filteredServices.length,
      tools: filteredServices.reduce((sum, service) => sum + (service.tools?.length || 0), 0),
      connected: count("Connected"),
      disconnected: count("Disconnected"),
      connecting: count("Connecting"),
      error: count("Error"),
    }
  }, [filteredServices])

  return {
    agentFilter,
    agentIds,
    filteredServices,
    query,
    setAgentFilter,
    setQuery,
    totals,
  }
}
