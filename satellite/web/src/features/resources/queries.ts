import { useQueries } from "@tanstack/react-query"

import { listInstanceResources, type ServiceInstance } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useInstanceResourcesQueries(instances: ServiceInstance[]) {
  return useQueries({
    queries: instances.map((instance) => ({
      queryKey: queryKeys.instanceResources(instance),
      queryFn: () => listInstanceResources(instance),
    })),
  })
}
