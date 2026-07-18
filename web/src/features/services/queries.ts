import { useQuery } from "@tanstack/react-query"

import {
  getInstanceAuthStatus,
  getServiceState,
  getServiceInstance,
  listInstancePrompts,
  listInstanceResourceTemplates,
  listInstanceResources,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useServiceDetailQuery(instanceId: string, enabled = false) {
  return useQuery({ enabled: enabled && Boolean(instanceId), queryKey: queryKeys.instance(instanceId), queryFn: () => getServiceInstance(instanceId) })
}

export function useServiceAuthQuery(instanceId: string) {
  return useQuery({
    enabled: Boolean(instanceId),
    queryKey: queryKeys.instanceAuth(instanceId),
    queryFn: () => getInstanceAuthStatus(instanceId),
  })
}

export function useServiceStatusQuery(instanceId: string) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.instanceStatus(instanceId),
    queryFn: () => getServiceState(instanceId).catch(() => null),
  })
}

export function useServiceResourcesQuery(instanceId: string) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.instanceResources(instanceId),
    queryFn: () => listInstanceResources(instanceId).catch(() => []),
  })
}

export function useServiceResourceTemplatesQuery(instanceId: string) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.instanceResourceTemplates(instanceId),
    queryFn: () => listInstanceResourceTemplates(instanceId).catch(() => []),
  })
}

export function useServicePromptsQuery(instanceId: string) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.instancePrompts(instanceId),
    queryFn: () => listInstancePrompts(instanceId).catch(() => []),
  })
}
