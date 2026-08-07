import { useQuery } from "@tanstack/react-query"

import {
  getInstanceAuthStatus,
  getServiceState,
  getServiceInstance,
  listInstancePrompts,
  listInstanceResourceTemplates,
  listInstanceResources,
  type ServiceAddress,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useServiceDetailQuery(addr: ServiceAddress, enabled = false) {
  return useQuery({ enabled: enabled && Boolean(addr.service_name), queryKey: queryKeys.instance(addr), queryFn: () => getServiceInstance(addr) })
}

export function useServiceAuthQuery(addr: ServiceAddress) {
  return useQuery({
    enabled: Boolean(addr.service_name),
    queryKey: queryKeys.instanceAuth(addr),
    queryFn: () => getInstanceAuthStatus(addr),
  })
}

export function useServiceStatusQuery(addr: ServiceAddress) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.instanceStatus(addr),
    queryFn: () => getServiceState(addr).catch(() => null),
  })
}

export function useServiceResourcesQuery(addr: ServiceAddress) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.instanceResources(addr),
    queryFn: () => listInstanceResources(addr).catch(() => []),
  })
}

export function useServiceResourceTemplatesQuery(addr: ServiceAddress) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.instanceResourceTemplates(addr),
    queryFn: () => listInstanceResourceTemplates(addr).catch(() => []),
  })
}

export function useServicePromptsQuery(addr: ServiceAddress) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.instancePrompts(addr),
    queryFn: () => listInstancePrompts(addr).catch(() => []),
  })
}
