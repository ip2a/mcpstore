import { useEffect, useMemo, useState } from "react"
import { ClipboardIcon, DatabaseIcon, RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import {
  buildDisplayCacheTree,
  extractCacheNamespace,
  getAllLayerNodes,
  LAYER_I18N_KEY,
  type CacheLayerId,
} from "@/features/cache/cache-model"
import { CacheCollectionNav, firstCollection } from "@/features/cache/cache-layer-browser"
import { CacheLayerContent, CacheOverviewContent } from "@/features/cache/cache-response-view"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SectionHeading } from "@/components/shared/section-heading"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useCacheHealthQuery, useCacheInspectQuery } from "@/features/cache/queries"
import { useI18n } from "@/lib/i18n-context"
import type { CacheBackend } from "@/lib/api"
import { cn } from "@/lib/utils"

const OVERVIEW_TAB = "overview"

type CacheTabId = "overview" | CacheLayerId

export function CacheView(props: {
  backend?: CacheBackend
  revision: number
  onRefreshDashboard: () => Promise<void>
  onSwitch: () => void
}) {
  const { t } = useI18n()
  const [refreshError, setRefreshError] = useState<string | null>(null)
  const [activeTab, setActiveTab] = useState<CacheTabId>(OVERVIEW_TAB)
  const [selectedCollection, setSelectedCollection] = useState("")
  const healthQuery = useCacheHealthQuery()
  const inspectQuery = useCacheInspectQuery()
  const healthReport = healthQuery.data
  const inspectReport = inspectQuery.data
  const tree = useMemo(
    () => buildDisplayCacheTree(inspectReport, healthReport),
    [inspectReport, healthReport],
  )
  const layerNodes = useMemo(() => getAllLayerNodes(tree), [tree])
  const selectedLayerNode = useMemo(
    () => layerNodes.find((layer) => layer.layer === activeTab) ?? layerNodes[0],
    [layerNodes, activeTab],
  )
  const activeCollectionNode = useMemo(
    () =>
      selectedLayerNode.collections.find(
        (collection) => collection.collection === selectedCollection,
      ) ?? null,
    [selectedLayerNode, selectedCollection],
  )
  const namespace =
    extractCacheNamespace(inspectReport, healthReport) ?? tree?.namespace ?? t("unknown")
  const backend = props.backend || tree?.backend || t("unknown")
  const namespaceLine = [namespace, backend].filter(Boolean).join(" · ")
  const headerLine = tree
    ? [
        namespaceLine,
        t("cacheLayerSummary", { types: tree.totalCollections, keys: tree.totalKeys }),
      ]
        .filter(Boolean)
        .join(" · ")
    : namespaceLine
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
      await props.onRefreshDashboard()
    } catch (err) {
      const message = err instanceof Error ? err.message : t("cacheLoadFailed")
      if (!inspectQuery.error) setRefreshError(message)
      toast.error(message)
    }
  }

  useEffect(() => {
    void loadCache()
  }, [props.revision])

  useEffect(() => {
    setActiveTab(OVERVIEW_TAB)
  }, [props.revision])

  useEffect(() => {
    setSelectedCollection(firstCollection(selectedLayerNode.collections))
  }, [selectedLayerNode.layer, selectedLayerNode.collections])

  const stateGuard =
    error && !hasData ? (
      <PageError
        title={t("cacheViewFailedToLoad", { view: t("cacheStorageTitle") })}
        message={errorMessage}
        onRefresh={loadCache}
      />
    ) : loading && !hasData ? (
      <PageSkeleton />
    ) : !tree ? (
      <PageSkeleton />
    ) : null

  return (
    <>
      <div className="min-w-0 shrink-0 pl-4">
        <h1 className="text-lg font-semibold">{t("cacheStorageTitle")}</h1>
        <p className="mt-1 truncate font-mono text-sm text-muted-foreground" title={headerLine}>
          {headerLine}
        </p>
      </div>

      <PanelCard className="@container flex h-full min-h-0 flex-1 flex-col overflow-hidden">
        <Tabs
          value={activeTab}
          onValueChange={(value) => setActiveTab(value as CacheTabId)}
          className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
        >
          <div className="flex min-w-0 shrink-0 items-center gap-3 pb-2">
            <TabsList variant="line" className="h-9 w-fit shrink-0 justify-start">
              <TabsTrigger value={OVERVIEW_TAB} title={t("overview")}>
                {t("overview")}
              </TabsTrigger>
              {layerNodes.map((layerNode) => {
                const label = t(LAYER_I18N_KEY[layerNode.layer])
                return (
                  <TabsTrigger key={layerNode.layer} value={layerNode.layer} title={label}>
                    {label}
                  </TabsTrigger>
                )
              })}
            </TabsList>
            <CacheActions
              className="ml-auto"
              loading={loading}
              canCopy={Boolean(inspectReport || healthReport)}
              onCopy={() => copyReport({ inspect: inspectReport, health: healthReport })}
              onRefresh={loadCache}
              onSwitch={props.onSwitch}
            />
          </div>

          <TabsContent
            value={OVERVIEW_TAB}
            className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
          >
            {stateGuard ?? (tree ? <CacheOverviewContent tree={tree} /> : null)}
          </TabsContent>

          {layerNodes.map((layerNode) => {
            const layerActive = activeTab === layerNode.layer
            return (
              <TabsContent
                key={layerNode.layer}
                value={layerNode.layer}
                className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
              >
                {stateGuard ?? (
                  <div className="grid min-h-0 min-w-0 flex-1 grid-cols-[minmax(240px,0.36fr)_minmax(0,1fr)] gap-4 [&>*]:min-h-0 [&>*]:min-w-0">
                    <div className="flex min-h-0 min-w-0 flex-col gap-3">
                      <SectionHeading
                        title={t("cacheCollections")}
                        titleAs="h2"
                        description={
                          layerNode.keyCount > 0
                            ? t("cacheLayerSummary", {
                                types: layerNode.typeCount,
                                keys: layerNode.keyCount,
                              })
                            : t("typesCount", { count: layerNode.typeCount })
                        }
                        descriptionPlacement="inline"
                        className="shrink-0 border-b-0 pb-0"
                      />
                      <CacheCollectionNav
                        collections={layerNode.collections}
                        value={layerActive ? selectedCollection : ""}
                        onValueChange={setSelectedCollection}
                      />
                    </div>
                    <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
                      <div className="flex min-w-0 shrink-0 items-center justify-between gap-3 border-b pb-2">
                        <strong
                          className="truncate text-sm font-medium"
                          title={activeCollectionNode?.type ?? t(LAYER_I18N_KEY[layerNode.layer])}
                        >
                          {activeCollectionNode?.type ?? t(LAYER_I18N_KEY[layerNode.layer])}
                        </strong>
                        {activeCollectionNode ? (
                          <span className="shrink-0 text-sm text-muted-foreground">
                            {t("keysCount", { count: activeCollectionNode.keyCount })}
                          </span>
                        ) : null}
                      </div>
                      <CacheLayerContent collection={activeCollectionNode} />
                    </div>
                  </div>
                )}
              </TabsContent>
            )
          })}
        </Tabs>
      </PanelCard>
    </>
  )
}

function CacheActions({
  loading,
  canCopy,
  onCopy,
  onRefresh,
  onSwitch,
  className,
}: {
  loading: boolean
  canCopy: boolean
  onCopy: () => void
  onRefresh: () => void
  onSwitch: () => void
  className?: string
}) {
  const { t } = useI18n()

  return (
    <div className={cn("flex shrink-0 gap-2", className)}>
      <Button size="sm" variant="outline" onClick={onSwitch}>
        <DatabaseIcon data-icon="inline-start" />
        {t("switchBackend")}
      </Button>
      {canCopy ? (
        <Button size="sm" variant="outline" onClick={onCopy}>
          <ClipboardIcon data-icon="inline-start" />
          {t("copy")}
        </Button>
      ) : null}
      <Button size="sm" variant="outline" onClick={onRefresh} disabled={loading}>
        <RefreshCwIcon data-icon="inline-start" />
        {t("refresh")}
      </Button>
    </div>
  )
}

async function copyReport(value: unknown) {
  await navigator.clipboard.writeText(JSON.stringify(value, null, 2))
  toast.success("Copied")
}
