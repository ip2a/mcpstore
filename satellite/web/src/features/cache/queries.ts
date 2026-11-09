import { useQuery } from "@tanstack/react-query"

import { cacheHealth, cacheInspect } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useCacheHealthQuery() {
  return useQuery({ queryKey: queryKeys.cacheHealth, queryFn: cacheHealth })
}

export function useCacheInspectQuery() {
  return useQuery({ queryKey: queryKeys.cacheInspect, queryFn: cacheInspect })
}
