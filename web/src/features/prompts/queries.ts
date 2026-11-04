import { useQueries } from "@tanstack/react-query"

import { listInstancePrompts, type ServiceInstance } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useInstancePromptsQueries(instances: ServiceInstance[]) {
  return useQueries({
    queries: instances.map((instance) => ({
      queryKey: queryKeys.instancePrompts(instance),
      queryFn: () => listInstancePrompts(instance),
    })),
  })
}
