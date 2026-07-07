import { useQuery } from "@tanstack/react-query"
import { listAgentTools, listTools } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useToolsQuery({ agentId, scope, serviceName }: { agentId: string; scope: string; serviceName?: string }) {
  return useQuery({
    enabled: false,
    queryKey: scope === "agent" && agentId ? queryKeys.agentTools(agentId, serviceName) : queryKeys.tools(serviceName),
    queryFn: () => (scope === "agent" && agentId ? listAgentTools(agentId, serviceName) : listTools(serviceName)),
  })
}
