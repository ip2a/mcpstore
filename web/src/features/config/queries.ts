import { useQuery } from "@tanstack/react-query"

import { showAgentConfig, showConfig } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useStoreConfigQuery() {
  return useQuery({ enabled: false, queryKey: queryKeys.config, queryFn: showConfig })
}

export function useAgentConfigQuery(agentId: string) {
  return useQuery({ enabled: false, queryKey: queryKeys.agentConfig(agentId), queryFn: () => showAgentConfig(agentId) })
}
