import { useQuery } from "@tanstack/react-query"

import { listAgentServices, listScopes, listServices, type ScopeView } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useAgentServicesQuery(agentId: string) {
  return useQuery({
    enabled: Boolean(agentId),
    queryKey: queryKeys.agentServices(agentId),
    queryFn: () => listAgentServices(agentId),
  })
}

/** Scope registry (root + store + each agent). Reference §17.3. */
export function useScopesQuery() {
  return useQuery({ queryKey: queryKeys.scopes, queryFn: () => listScopes() })
}

/** Service list under a read view (root aggregate / store / agent). */
export function useScopeServicesQuery(view: ScopeView) {
  return useQuery({
    enabled: view.type !== "agent" || Boolean(view.agent_id),
    queryKey: queryKeys.scopeServices(view),
    queryFn: () => listServices(view),
  })
}
