import { useQuery } from "@tanstack/react-query"

import { serviceInfo, serviceStatus } from "@/lib/api"
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
