import { useMutation, useQuery } from "@tanstack/react-query"

import { getMeta, updateSettings } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function useSettingsMetaQuery(open: boolean) {
  return useQuery({
    enabled: open,
    queryKey: queryKeys.meta,
    queryFn: getMeta,
  })
}

export function useUpdateSettingsMutation() {
  return useMutation({ mutationKey: queryKeys.settings, mutationFn: updateSettings })
}
