import { useQuery } from "@tanstack/react-query"

import { kvHealth, kvInspect } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useKvHealthQuery() {
  return useQuery({ queryKey: queryKeys.kvHealth, queryFn: kvHealth })
}

export function useKvInspectQuery() {
  return useQuery({ queryKey: queryKeys.kvInspect, queryFn: kvInspect })
}
