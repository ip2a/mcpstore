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

/** 作用域注册表（root + store + 各 agent）。文档 §17.3。 */
export function useScopesQuery() {
  return useQuery({ queryKey: queryKeys.scopes, queryFn: () => listScopes() })
}

/** 某个读视图下的服务列表（root 聚合 / store / agent）。 */
export function useScopeServicesQuery(view: ScopeView) {
  return useQuery({
    enabled: view.type !== "agent" || Boolean(view.agent_id),
    queryKey: queryKeys.scopeServices(view),
    queryFn: () => listServices(view),
  })
}
