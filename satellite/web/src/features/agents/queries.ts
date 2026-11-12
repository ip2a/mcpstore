import { useQuery, type Query } from "@tanstack/react-query"

import {
  listAgentServices,
  listScopes,
  listServices,
  type ScopeView,
  type ServiceInstance,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useAgentServicesQuery(agentId: string) {
  return useQuery({
    enabled: Boolean(agentId),
    queryKey: queryKeys.agentServices(agentId),
    queryFn: () => listAgentServices(agentId),
  })
}

/** Scope registry (root + store + each agent). Doc §17.3. */
export function useScopesQuery() {
  return useQuery({ queryKey: queryKeys.scopes, queryFn: () => listScopes() })
}

type ScopeServicesQuery = Query<ServiceInstance[], Error, ServiceInstance[], readonly unknown[]>

/** Service list under a read view (root aggregation / store / agent). */
export function useScopeServicesQuery(
  view: ScopeView,
  options?: {
    refetchInterval?:
      | number
      | false
      | ((query: ScopeServicesQuery) => number | false | undefined)
  },
) {
  return useQuery({
    enabled: view.type !== "agent" || Boolean(view.agent_id),
    queryKey: queryKeys.scopeServices(view),
    queryFn: () => listServices(view),
    refetchInterval: options?.refetchInterval,
  })
}
