import { useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import type { ResetTarget } from "@/features/config/reset-config-dialog"
import type { ServiceAddress } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useAppQueryRefreshers() {
  const queryClient = useQueryClient()
  const [kvRevision, setKvRevision] = useState(0)
  const [serviceDetailRevision, setServiceDetailRevision] = useState(0)

  async function refreshInstanceQueries(addr: ServiceAddress) {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.instances }),
      queryClient.invalidateQueries({ queryKey: queryKeys.instance(addr) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.instanceStatus(addr) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.instanceTools(addr) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.instanceResources(addr) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.instanceResourceTemplates(addr) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.instancePrompts(addr) }),
      addr.scope.type === "agent"
        ? queryClient.invalidateQueries({ queryKey: queryKeys.agentServices(addr.scope.agent_id) })
        : Promise.resolve(),
    ])
    setServiceDetailRevision((value) => value + 1)
  }

  async function refreshServiceRegistryQueries() {
    await Promise.all([
      queryClient.refetchQueries({ queryKey: queryKeys.instances }),
      queryClient.refetchQueries({ queryKey: queryKeys.agents }),
    ])
  }

  async function refreshAgentQueries(agentId: string) {
    await Promise.all([
      queryClient.refetchQueries({ queryKey: queryKeys.agents }),
      queryClient.refetchQueries({ queryKey: queryKeys.agent(agentId) }),
      queryClient.refetchQueries({ queryKey: queryKeys.agentServices(agentId) }),
    ])
  }

  async function refreshKvQueries() {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.health }),
      queryClient.invalidateQueries({ queryKey: queryKeys.kvHealth }),
      queryClient.invalidateQueries({ queryKey: queryKeys.kvInspect }),
    ])
    setKvRevision((value) => value + 1)
  }

  async function refreshConfigQueries(target: ResetTarget) {
    await queryClient.invalidateQueries({
      queryKey: target.scope === "store" ? queryKeys.config : queryKeys.agentConfig(target.agentId),
    })
  }

  return {
    kvRevision,
    refreshAgentQueries,
    refreshKvQueries,
    refreshConfigQueries,
    refreshInstanceQueries,
    refreshServiceRegistryQueries,
    serviceDetailRevision,
  }
}
