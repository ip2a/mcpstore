import { useQueries } from "@tanstack/react-query"

import { listAgentServices, listInstanceTools, type ServiceInstance, type ToolInfo, type ToolVisibilityFilter } from "@/lib/api"
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

export function useScopeToolCounts(agentIds: string[], storeServices: ServiceInstance[]) {
  const agentServiceQueries = useQueries({
    queries: agentIds.map((agentId) => ({
      queryKey: queryKeys.agentServices(agentId),
      queryFn: () => listAgentServices(agentId),
    })),
  })
  const scopes = [
    { id: "store", services: storeServices },
    ...agentIds.map((id, index) => ({ id, services: agentServiceQueries[index]?.data || [] })),
  ]
  const instances = scopes.flatMap(({ id, services }) => services.map((service) => ({ id, service })))
  const toolQueries = useQueries({
    queries: instances.map(({ service }) => ({
      queryKey: [...queryKeys.instanceTools(service.instance_id), "available"],
      queryFn: () => listInstanceTools(service.instance_id, "available"),
    })),
  })

  return Object.fromEntries(scopes.map(({ id, services }) => [
    id,
    {
      services: services.length,
      tools: instances.reduce((count, instance, index) => count + (instance.id === id ? toolQueries[index]?.data?.length || 0 : 0), 0),
    },
  ])) as Record<string, { services: number; tools: number }>
}
