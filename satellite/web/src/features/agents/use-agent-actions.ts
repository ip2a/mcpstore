import { declareAgentServiceScope, removeServiceScope } from "@/lib/api"

type RunAction = (
  label: string,
  action: () => Promise<unknown>,
  onSuccess?: () => Promise<void> | void,
) => Promise<void>

export function useAgentActions({
  refreshAgentQueries,
  refreshServiceRegistryQueries,
  runAction,
}: {
  refreshAgentQueries: (agentId: string) => Promise<void>
  refreshServiceRegistryQueries: () => Promise<void>
  runAction: RunAction
}) {
  function declareServiceScope(agentId: string, serviceName: string) {
    return runAction(
      `declare-scope:${agentId}:${serviceName}`,
      () => declareAgentServiceScope(agentId, serviceName),
      async () => {
        await Promise.all([refreshAgentQueries(agentId), refreshServiceRegistryQueries()])
      },
    )
  }

  function removeAgentServiceScope(agentId: string, serviceName: string) {
    return runAction(
      `remove-scope:${agentId}:${serviceName}`,
      () => removeServiceScope(serviceName, { type: "agent", agent_id: agentId }),
      async () => {
        await Promise.all([refreshAgentQueries(agentId), refreshServiceRegistryQueries()])
      },
    )
  }

  return {
    declareServiceScope,
    removeAgentServiceScope,
  }
}
