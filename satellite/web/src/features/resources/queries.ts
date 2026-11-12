import { useQuery, useQueries } from "@tanstack/react-query"

import { listInstanceResources, listScopeResources, type ServiceInstance } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

/** Per-instance resources query — detail views only; the catalog page uses the scope aggregate. */
export function useInstanceResourcesQueries(instances: ServiceInstance[]) {
  return useQueries({
    queries: instances.map((instance) => ({
      queryKey: queryKeys.instanceResources(instance),
      queryFn: () => listInstanceResources(instance),
      // Listing no longer auto-connects (backend); default retry is safe.
    })),
  })
}

/** One request for the whole scope's resource catalog; entries carry their instance. */
export function useScopeResourcesQuery(scope: "store" | "agent", agentId: string) {
  return useQuery({
    queryKey: queryKeys.scopeResources(scope, agentId),
    queryFn: () => listScopeResources(scope, agentId),
  })
}
