import { useQueries } from "@tanstack/react-query"

import {
  listAgentServices,
  listInstanceTools,
  type ServiceInstance,
  type ToolInfo,
  type ToolVisibilityFilter,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type InstanceTool = {
  instance: ServiceInstance
  tool: ToolInfo
}

export function useInstanceToolsQueries(
  instances: ServiceInstance[],
  filter: ToolVisibilityFilter,
) {
  return useQueries({
    queries: instances.map((instance) => ({
      queryKey: [...queryKeys.instanceTools(instance), filter],
      queryFn: () => listInstanceTools(instance, filter),
      refetchInterval: 30_000,
      // tools/list is KV-only on the backend — never connects — so default
      // retry/focus behavior cannot trigger connect storms.
      meta: {
        instance,
      },
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
  const instances = scopes.flatMap(({ id, services }) =>
    services.map((service) => ({ id, service })),
  )
  const toolQueries = useQueries({
    queries: instances.map(({ service }) => ({
      queryKey: [...queryKeys.instanceTools(service), "available"],
      queryFn: () => listInstanceTools(service, "available"),
    })),
  })

  return Object.fromEntries(
    scopes.map(({ id, services }) => [
      id,
      {
        services: services.length,
        tools: instances.reduce(
          (count, instance, index) =>
            count + (instance.id === id ? toolQueries[index]?.data?.length || 0 : 0),
          0,
        ),
      },
    ]),
  ) as Record<string, { services: number; tools: number }>
}
