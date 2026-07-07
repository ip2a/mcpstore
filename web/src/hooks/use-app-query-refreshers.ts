import { useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import type { ResetTarget } from "@/features/config/config-view"
import { queryKeys } from "@/lib/query-keys"

export function useAppQueryRefreshers() {
  const queryClient = useQueryClient()
  const [cacheRevision, setCacheRevision] = useState(0)
  const [serviceDetailRevision, setServiceDetailRevision] = useState(0)

  async function refreshServiceQueries(serviceName: string, agentId?: string) {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.services }),
      queryClient.invalidateQueries({ queryKey: queryKeys.service(serviceName) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.serviceStatus(serviceName) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.toolsRoot }),
      queryClient.invalidateQueries({ queryKey: queryKeys.agents }),
      agentId ? queryClient.invalidateQueries({ queryKey: queryKeys.agent(agentId) }) : Promise.resolve(),
    ])
    setServiceDetailRevision((value) => value + 1)
  }

  async function refreshServiceRegistryQueries() {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.services }),
      queryClient.invalidateQueries({ queryKey: queryKeys.toolsRoot }),
      queryClient.invalidateQueries({ queryKey: queryKeys.agents }),
    ])
  }

  async function refreshAgentQueries(agentId: string) {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.agents }),
      queryClient.invalidateQueries({ queryKey: queryKeys.agent(agentId) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.toolsRoot }),
      queryClient.invalidateQueries({ queryKey: queryKeys.agentToolsRoot(agentId) }),
    ])
  }

  async function refreshCacheQueries() {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.health }),
      queryClient.invalidateQueries({ queryKey: queryKeys.cacheHealth }),
      queryClient.invalidateQueries({ queryKey: queryKeys.cacheInspect }),
    ])
    setCacheRevision((value) => value + 1)
  }

  async function refreshConfigQueries(target: ResetTarget) {
    await queryClient.invalidateQueries({ queryKey: target.scope === "store" ? queryKeys.config : queryKeys.agentConfig(target.agentId) })
  }

  return {
    cacheRevision,
    refreshAgentQueries,
    refreshCacheQueries,
    refreshConfigQueries,
    refreshServiceQueries,
    refreshServiceRegistryQueries,
    serviceDetailRevision,
  }
}
