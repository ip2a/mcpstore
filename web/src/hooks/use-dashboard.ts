import { useCallback, useEffect, useMemo, useState } from "react"
import { health, listAgents, listServices, type AgentItem, type CacheBackend, type ServiceEntry } from "@/lib/api"

export function useDashboard() {
  const [services, setServices] = useState<ServiceEntry[]>([])
  const [agents, setAgents] = useState<AgentItem[]>([])
  const [backend, setBackend] = useState<CacheBackend | undefined>()
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const [nextHealth, nextServices, nextAgents] = await Promise.all([
        health(),
        listServices(),
        listAgents().catch(() => []),
      ])
      setBackend(nextHealth.backend)
      setServices(nextServices.sort((a, b) => a.name.localeCompare(b.name)))
      setAgents(nextAgents)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load mcpstore")
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const agentMap = useMemo(() => {
    const map = new Map<string, string>()
    for (const agent of agents) {
      const agentId = String(agent.agent_id || agent.id || "")
      const serviceNames = agent.services || agent.service_names || []
      for (const name of serviceNames) {
        map.set(String(name), agentId)
      }
    }
    return map
  }, [agents])

  return { services, agents, agentMap, backend, loading, error, refresh }
}
