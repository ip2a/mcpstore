import { assignService, unassignService } from "@/lib/api"

type RunAction = (
  label: string,
  action: () => Promise<unknown>,
  onSuccess?: () => Promise<void> | void,
) => Promise<void>

export function useAgentActions({
  refreshAgentQueries,
  refreshServiceQueries,
  runAction,
}: {
  refreshAgentQueries: (agentId: string) => Promise<void>
  refreshServiceQueries: (serviceName: string, agentId?: string) => Promise<void>
  runAction: RunAction
}) {
  function assignServiceToAgent(agentId: string, serviceName: string) {
    return runAction(`assign:${agentId}:${serviceName}`, () => assignService(agentId, serviceName), async () => {
      await Promise.all([refreshAgentQueries(agentId), refreshServiceQueries(serviceName, agentId)])
    })
  }

  function unassignServiceFromAgent(agentId: string, serviceName: string) {
    return runAction(`unassign:${agentId}:${serviceName}`, () => unassignService(agentId, serviceName), async () => {
      await Promise.all([refreshAgentQueries(agentId), refreshServiceQueries(serviceName, agentId)])
    })
  }

  return {
    assignServiceToAgent,
    unassignServiceFromAgent,
  }
}
