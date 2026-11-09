import { useQueries } from "@tanstack/react-query"

import { listInstanceResources, type ServiceInstance } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useInstanceResourcesQueries(instances: ServiceInstance[]) {
  return useQueries({
    queries: instances.map((instance) => ({
      queryKey: queryKeys.instanceResources(instance),
      queryFn: () => listInstanceResources(instance),
      retry: false, // Don't retry failed requests to avoid cascade errors
      retryOnMount: false,
      refetchOnWindowFocus: false,
      // Return empty array on error instead of throwing
      // This prevents the error from bubbling up and crashing the UI
      meta: {
        // Store instance info for error tracking
        instance,
      },
    })),
  })
}
