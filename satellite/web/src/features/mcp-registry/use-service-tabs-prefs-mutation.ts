import { useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import type { ServiceTabsPrefs } from "@/features/mcp-registry/service-tabs-prefs"
import { useUpdateSettingsMutation } from "@/features/settings/queries"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"

/** Persist service-tab prefs from filter dialogs; invalidates meta so tabs refresh. */
export function useServiceTabsPrefsMutation() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const mutation = useUpdateSettingsMutation()

  async function saveServiceTabsPrefs(prefs: ServiceTabsPrefs) {
    try {
      await mutation.mutateAsync({
        service_tabs: {
          sort: prefs.sort,
          show_disconnected: prefs.showDisconnected,
        },
      })
      await queryClient.invalidateQueries({ queryKey: queryKeys.meta })
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("saveFailed"))
    }
  }

  return { saveServiceTabsPrefs, saving: mutation.isPending }
}
