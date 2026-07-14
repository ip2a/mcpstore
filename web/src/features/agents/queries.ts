import { useQuery } from "@tanstack/react-query"

import { listAgentServices } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useAgentServicesQuery(agentId: string) {
  return useQuery({
    enabled: Boolean(agentId),
    queryKey: queryKeys.agentServices(agentId),
    queryFn: () => listAgentServices(agentId),
  })
}
