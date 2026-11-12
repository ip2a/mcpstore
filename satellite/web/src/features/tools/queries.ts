import { useQuery, useQueries } from "@tanstack/react-query"

import {
  listInstanceTools,
  listScopeTools,
  type ServiceInstance,
  type ToolInfo,
  type ToolVisibilityFilter,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

/** Per-instance tools query — detail views only; the catalog page uses the scope aggregate. */
export function useInstanceToolsQueries(
  instances: ServiceInstance[],
  filter: ToolVisibilityFilter,
) {
  return useQueries({
    queries: instances.map((instance) => ({
      queryKey: [...queryKeys.instanceTools(instance), filter],
      queryFn: () => listInstanceTools(instance, filter),
      // tools/list is KV-only on the backend — never connects — so default
      // retry/focus behavior cannot trigger connect storms.
    })),
  })
}

/** One request for the whole scope's tool catalog; entries carry their instance. */
export function useScopeToolsQuery(
  scope: "store" | "agent",
  agentId: string,
  filter: ToolVisibilityFilter,
) {
  return useQuery({
    queryKey: queryKeys.scopeTools(scope, agentId, filter),
    queryFn: () => listScopeTools(scope, agentId, filter),
    refetchInterval: 30_000,
    // The aggregate is KV-only on the backend — never connects — so default
    // retry/focus behavior cannot trigger connect storms.
  })
}
