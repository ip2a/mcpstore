import { useQuery, useQueries } from "@tanstack/react-query"

import { listInstancePrompts, listScopePrompts, type ServiceInstance } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

/** Per-instance prompts query — detail views only; the catalog page uses the scope aggregate. */
export function useInstancePromptsQueries(instances: ServiceInstance[]) {
  return useQueries({
    queries: instances.map((instance) => ({
      queryKey: queryKeys.instancePrompts(instance),
      queryFn: () => listInstancePrompts(instance),
      // Listing no longer auto-connects (backend), so a retry can never
      // trigger a connect storm — default retry/focus behavior is safe.
    })),
  })
}

/** One request for the whole scope's prompt catalog; entries carry their instance. */
export function useScopePromptsQuery(scope: "store" | "agent", agentId: string) {
  return useQuery({
    queryKey: queryKeys.scopePrompts(scope, agentId),
    queryFn: () => listScopePrompts(scope, agentId),
  })
}
