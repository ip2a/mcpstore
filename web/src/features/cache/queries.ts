import { useQuery } from "@tanstack/react-query"

import { cacheHealth, cacheInspect } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useCacheHealthQuery() {
  return useQuery({ enabled: false, queryKey: queryKeys.cacheHealth, queryFn: cacheHealth })
}

export function useCacheInspectQuery() {
  return useQuery({ enabled: false, queryKey: queryKeys.cacheInspect, queryFn: cacheInspect })
}
