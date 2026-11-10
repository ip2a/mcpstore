import { useQueries } from "@tanstack/react-query"

import { listInstancePrompts, type ServiceInstance } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useInstancePromptsQueries(instances: ServiceInstance[]) {
  return useQueries({
    queries: instances.map((instance) => ({
      queryKey: queryKeys.instancePrompts(instance),
      queryFn: () => listInstancePrompts(instance),
      // Listing no longer auto-connects (backend), so a retry can never
      // trigger a connect storm — default retry/focus behavior is safe and a
      // transient failure no longer drops an instance from the catalog.
      meta: {
        instance,
      },
    })),
  })
}
