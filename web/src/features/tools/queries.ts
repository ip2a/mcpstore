import { useQueries } from "@tanstack/react-query"

import { listInstanceTools, type ServiceInstance, type ToolInfo, type ToolVisibilityFilter } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type InstanceTool = {
  instance: ServiceInstance
  tool: ToolInfo
}

export function useInstanceToolsQueries(instances: ServiceInstance[], filter: ToolVisibilityFilter) {
  return useQueries({
    queries: instances.map((instance) => ({
      queryKey: [...queryKeys.instanceTools(instance.instance_id), filter],
      queryFn: () => listInstanceTools(instance.instance_id, filter),
    })),
  })
}
