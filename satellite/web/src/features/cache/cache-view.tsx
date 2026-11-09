import { useMemo, useState } from "react"
import { toast } from "sonner"

import {
  buildDisplayCacheTree,
  getAllLayerNodes,
} from "@/features/cache/cache-model"
import { CacheLayerContent } from "@/features/cache/cache-response-view"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { useCacheHealthQuery, useCacheInspectQuery } from "@/features/cache/queries"
import { useI18n } from "@/lib/i18n-context"

/** Cache keys table for embedding (e.g. Traces → Cache tab). */
export function CachePanel() {
  const { t } = useI18n()
  const [refreshError, setRefreshError] = useState<string | null>(null)
  const healthQuery = useCacheHealthQuery()
  const inspectQuery = useCacheInspectQuery()
  const tree = useMemo(
    () => buildDisplayCacheTree(inspectQuery.data, healthQuery.data),
    [inspectQuery.data, healthQuery.data],
  )
  const layerNodes = useMemo(() => getAllLayerNodes(tree), [tree])
  const error = inspectQuery.error || refreshError
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : t("cacheLoadFailed")
  const loading = healthQuery.isFetching || inspectQuery.isFetching
  const hasData = Boolean(tree)

  async function loadCache() {
    try {
      setRefreshError(null)
      const [health, inspect] = await Promise.all([healthQuery.refetch(), inspectQuery.refetch()])
      if (inspect.error) throw inspect.error
      if (health.error && !inspect.data) throw health.error
    } catch (err) {
      const message = err instanceof Error ? err.message : t("cacheLoadFailed")
      if (!inspectQuery.error) setRefreshError(message)
      toast.error(message)
    }
  }

  if (error && !hasData) {
    return (
      <div className="px-4 py-3">
        <PageError
          title={t("cacheViewFailedToLoad", { view: t("cache") })}
          message={errorMessage}
          onRefresh={loadCache}
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
      <CacheLayerContent layers={layerNodes} />
    </div>
  )
}
