import { useMemo, useState } from "react"
import { toast } from "sonner"

import {
  buildDisplayKvTree,
  getAllLayerNodes,
} from "@/features/kv/kv-model"
import { KvLayerContent } from "@/features/kv/kv-response-view"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { useKvHealthQuery, useKvInspectQuery } from "@/features/kv/queries"
import { useI18n } from "@/lib/i18n-context"

/** KV keys table for embedding (e.g. Traces → KV tab). */
export function KvPanel() {
  const { t } = useI18n()
  const [refreshError, setRefreshError] = useState<string | null>(null)
  const healthQuery = useKvHealthQuery()
  const inspectQuery = useKvInspectQuery()
  const tree = useMemo(
    () => buildDisplayKvTree(inspectQuery.data, healthQuery.data),
    [inspectQuery.data, healthQuery.data],
  )
  const layerNodes = useMemo(() => getAllLayerNodes(tree), [tree])
  const error = inspectQuery.error || refreshError
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : t("kvLoadFailed")
  const loading = healthQuery.isFetching || inspectQuery.isFetching
  const hasData = Boolean(tree)

  async function loadKv() {
    try {
      setRefreshError(null)
      const [health, inspect] = await Promise.all([healthQuery.refetch(), inspectQuery.refetch()])
      if (inspect.error) throw inspect.error
      if (health.error && !inspect.data) throw health.error
    } catch (err) {
      const message = err instanceof Error ? err.message : t("kvLoadFailed")
      if (!inspectQuery.error) setRefreshError(message)
      toast.error(message)
    }
  }

  if (error && !hasData) {
    return (
      <div className="px-4 py-3">
        <PageError
          title={t("kvViewFailedToLoad", { view: t("kv") })}
          message={errorMessage}
          onRefresh={loadKv}
        />
      </div>
    )
  }

  if ((loading && !hasData) || !tree) {
    return (
      <div className="px-4 py-3">
        <PageSkeleton />
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden px-4 pt-2">
      <KvLayerContent layers={layerNodes} />
    </div>
  )
}
