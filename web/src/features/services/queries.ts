import { useQuery } from "@tanstack/react-query"

import { listPrompts, listResourceTemplates, listResources, serviceInfo, serviceStatus } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useServiceDetailQuery(serviceName: string) {
  return useQuery({ enabled: false, queryKey: queryKeys.service(serviceName), queryFn: () => serviceInfo(serviceName) })
}

export function useServiceStatusQuery(serviceName: string) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.serviceStatus(serviceName),
    queryFn: () => serviceStatus(serviceName).catch(() => null),
  })
}

export function useServiceResourcesQuery(serviceName: string) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.serviceResources(serviceName),
    queryFn: () => listResources(serviceName).catch(() => []),
  })
}

export function useServiceResourceTemplatesQuery(serviceName: string) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.serviceResourceTemplates(serviceName),
    queryFn: () => listResourceTemplates(serviceName).catch(() => []),
  })
}

export function useServicePromptsQuery(serviceName: string) {
  return useQuery({
    enabled: false,
    queryKey: queryKeys.servicePrompts(serviceName),
    queryFn: () => listPrompts(serviceName).catch(() => []),
  })
}
