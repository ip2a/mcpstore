import { useEffect, useMemo, useState } from "react"
import type { LucideIcon } from "lucide-react"
import {
  ActivityIcon,
  ClipboardIcon,
  DatabaseIcon,
  HistoryIcon,
  LinkIcon,
  PackageIcon,
  RefreshCwIcon,
} from "lucide-react"
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
import { CatalogTabTrigger, CatalogTabsList } from "@/components/shared/catalog-tabs-list"
import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SectionHeading } from "@/components/shared/section-heading"
import { TwoPanePage } from "@/components/shared/two-pane-page"
import { Button } from "@/components/ui/button"
import { Tabs } from "@/components/ui/tabs"
import { useCacheHealthQuery, useCacheInspectQuery } from "@/features/cache/queries"
import { useI18n } from "@/lib/i18n-context"
import { type CacheBackend } from "@/lib/api"
import { cn } from "@/lib/utils"

const LAYER_TAB_ICONS: Record<CacheLayerId, LucideIcon> = {
  entity: PackageIcon,
  relations: LinkIcon,
  state: ActivityIcon,
  event: HistoryIcon,
}

type RightPaneView = "overview" | "layer"

export function CacheView(props: {
  backend?: CacheBackend
  revision: number
  onRefreshDashboard: () => Promise<void>
  onSwitch: () => void
}) {
  const { t } = useI18n()
  const [refreshError, setRefreshError] = useState<string | null>(null)
  const [rightPaneView, setRightPaneView] = useState<RightPaneView>("overview")
  const [selectedLayer, setSelectedLayer] = useState<CacheLayerId>("entity")
  const [selectedCollection, setSelectedCollection] = useState("")
  const healthQuery = useCacheHealthQuery()
  const inspectQuery = useCacheInspectQuery()
  const healthReport = healthQuery.data
  const inspectReport = inspectQuery.data
  const tree = useMemo(() => buildDisplayCacheTree(inspectReport, healthReport), [inspectReport, healthReport])
  const layerNodes = useMemo(() => getAllLayerNodes(tree), [tree])
  const selectedLayerNode = useMemo(
    () => layerNodes.find((layer) => layer.layer === selectedLayer) ?? layerNodes[0],
    [layerNodes, selectedLayer],
  )
  const activeCollectionNode = useMemo(
    () => selectedLayerNode.collections.find((collection) => collection.collection === selectedCollection) ?? null,
    [selectedLayerNode, selectedCollection],
  )
  const namespace = extractCacheNamespace(inspectReport, healthReport) ?? tree?.namespace ?? t("unknown")
  const backend = props.backend || tree?.backend || t("unknown")
  const namespaceLine = [namespace, backend].filter(Boolean).join(" · ")
  const error = inspectQuery.error || refreshError
  const errorMessage = error instanceof Error ? error.message : error ? String(error) : t("cacheLoadFailed")
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
    setRightPaneView("overview")
  }, [props.revision])

  useEffect(() => {
    setSelectedCollection(firstCollection(selectedLayerNode.collections))
  }, [selectedLayer, selectedLayerNode.collections])

  return (
    <TwoPanePage variant="full" className="h-full min-h-0 flex-1 gap-4">
      <PanelCard className="@container flex h-full min-h-0 flex-col">
        <section className="flex flex-col gap-3 border-b pb-4">
          <div className="min-w-0">
            <p className="font-mono text-xs uppercase text-muted-foreground">{t("cache")}</p>
            <button
              type="button"
              onClick={() => setRightPaneView("overview")}
              className={cn(
                "mt-1 block max-w-full cursor-pointer truncate border-0 bg-transparent p-0 text-left text-lg font-semibold underline-offset-4 outline-none transition-opacity",
                "hover:underline active:opacity-70",
              )}
              title={t("cacheStorageTitle")}
            >
              {t("cacheStorageTitle")}
            </button>
            <p className="mt-1 truncate font-mono text-xs text-muted-foreground" title={namespaceLine}>
              {namespaceLine}
            </p>
          </div>
        </section>

        <Tabs
          value={selectedLayer}
          onValueChange={(value) => {
            setSelectedLayer(value as CacheLayerId)
            setRightPaneView("layer")
          }}
          className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden pt-3"
        >
          <CatalogTabsList>
            {layerNodes.map((layerNode) => {
              const Icon = LAYER_TAB_ICONS[layerNode.layer]
              const label = t(LAYER_I18N_KEY[layerNode.layer])
              return (
                <CatalogTabTrigger key={layerNode.layer} value={layerNode.layer} label={label} title={label}>
                  <Icon />
                </CatalogTabTrigger>
              )
            })}
          </CatalogTabsList>

          <SectionHeading
            title={t("cacheCollections")}
            titleAs="h2"
            description={
              selectedLayerNode.keyCount > 0
                ? t("cacheLayerSummary", {
                    types: selectedLayerNode.typeCount,
                    keys: selectedLayerNode.keyCount,
                  })
                : t("typesCount", { count: selectedLayerNode.typeCount })
            }
            descriptionPlacement="inline"
            className="shrink-0 border-b-0 pb-0"
          />
          <CacheCollectionNav
            collections={selectedLayerNode.collections}
            value={rightPaneView === "layer" ? selectedCollection : ""}
            onValueChange={(value) => {
              setSelectedCollection(value)
              setRightPaneView("layer")
            }}
          />
        </Tabs>
      </PanelCard>

      <PanelCard variant="plain" className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
        <CachePreviewHeader
          loading={loading}
          title={
            rightPaneView === "overview"
              ? t("cacheStorageTitle")
              : (activeCollectionNode?.type ?? t(LAYER_I18N_KEY[selectedLayerNode.layer]))
          }
          countLabel={
            rightPaneView === "layer"
              ? activeCollectionNode
                ? t("keysCount", { count: activeCollectionNode.keyCount })
                : t("cacheLayerSummary", {
                    types: selectedLayerNode.typeCount,
                    keys: selectedLayerNode.keyCount,
                  })
              : tree
                ? t("keysCount", { count: tree.totalKeys })
                : undefined
          }
          onCopy={
            inspectReport || healthReport
              ? () => copyReport({ inspect: inspectReport, health: healthReport })
              : undefined
          }
          onRefresh={loadCache}
          onSwitch={props.onSwitch}
        />

        {rightPaneView === "overview" && tree ? (
          <MetricGrid columns="four">
            <MetricTile variant="compact" label={t("cacheNamespace")} value={tree.namespace} title={tree.namespace} />
            <MetricTile
              variant="compact"
              label={t("backend")}
              value={props.backend || tree.backend || t("unknown")}
              hint={t("currentStorageBackend")}
            />
            <MetricTile
              variant="compact"
              label={t("cacheCollections")}
              value={String(tree.totalCollections)}
              hint={t("cacheActiveTypesHint")}
            />
            <MetricTile
              variant="compact"
              label={t("cacheTotalKeys")}
              value={String(tree.totalKeys)}
              hint={t("cacheTotalKeysHint")}
            />
          </MetricGrid>
        ) : null}

        {rightPaneView === "overview" ? (
          <ScrollPane className="flex-1">
            {error && !hasData ? (
              <PageError title={t("cacheViewFailedToLoad", { view: t("cacheStorageTitle") })} message={errorMessage} onRefresh={loadCache} />
            ) : loading && !hasData ? (
              <PageSkeleton />
            ) : !tree ? (
              <PageSkeleton />
            ) : (
              <CacheOverviewContent tree={tree} />
            )}
          </ScrollPane>
        ) : (
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            {error && !hasData ? (
              <PageError title={t("cacheViewFailedToLoad", { view: t("cacheStorageTitle") })} message={errorMessage} onRefresh={loadCache} />
            ) : loading && !hasData ? (
              <PageSkeleton />
            ) : !tree ? (
              <PageSkeleton />
            ) : (
              <CacheLayerContent collection={activeCollectionNode} />
            )}
          </div>
        )}
      </PanelCard>
    </TwoPanePage>
  )
}

function CachePreviewHeader({
  loading,
  onCopy,
  onRefresh,
  onSwitch,
  title,
  countLabel,
}: {
  loading: boolean
  onCopy?: () => void
  onRefresh: () => void
  onSwitch: () => void
  title: string
  countLabel?: string
}) {
  const { t } = useI18n()

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-b pb-2">
      <div className="flex min-w-0 flex-1 items-center justify-between gap-3">
        <strong className="truncate text-sm font-medium" title={title}>
          {title}
        </strong>
        {countLabel ? (
          <span className="shrink-0 text-sm text-muted-foreground">{countLabel}</span>
        ) : null}
      </div>
      <div className="flex shrink-0 flex-wrap justify-end gap-2">
        <Button size="sm" variant="outline" onClick={onSwitch}>
          <DatabaseIcon data-icon="inline-start" />
          {t("switchBackend")}
        </Button>
        {onCopy ? (
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
    </div>
  )
}

async function copyReport(value: unknown) {
  await navigator.clipboard.writeText(JSON.stringify(value, null, 2))
  toast.success("Copied")
}
