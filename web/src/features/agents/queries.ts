import { useQuery } from "@tanstack/react-query"

import { listAgentServices, listAgentTools } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useAgentServicesQuery(agentId: string) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.agentServices(agentId),
    queryFn: () => listAgentServices(agentId),
  })
}

export function useAgentToolsQuery(agentId: string) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.agentTools(agentId),
    queryFn: () => listAgentTools(agentId),
  })
}
