import { useQuery } from "@tanstack/react-query"

import {
  getInstanceAuthStatus,
  getServiceState,
  getServiceInstance,
  listInstancePrompts,
  listInstanceResourceTemplates,
  listInstanceResources,
  listInstanceTools,
  type ServiceAddress,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useServiceDetailQuery(addr: ServiceAddress, enabled = false) {
  return useQuery({
    enabled: enabled && Boolean(addr.service_name),
    queryKey: queryKeys.instance(addr),
    queryFn: () => getServiceInstance(addr),
  })
}

/// Override-projected tool list (GET /tools/list): display_name/description
/// projected, raw name kept in `tool_name`. KV-backed, so it works for
/// stopped services too.
export function useServiceProjectedToolsQuery(addr: ServiceAddress) {
  return useQuery({
    enabled: Boolean(addr.service_name),
    queryKey: queryKeys.instanceTools(addr),
    queryFn: () => listInstanceTools(addr).catch(() => []),
  })
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
