import { SectionHeading } from "@/components/shared/section-heading"
import { Badge } from "@/components/ui/badge"
import { CacheCollectionDetail } from "@/features/cache/cache-layer-browser"
import { LAYER_I18N_KEY, type CacheCollectionNode, type CacheTree } from "@/features/cache/cache-model"
import { useI18n } from "@/lib/i18n-context"

function ScalarField({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-1">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="break-all font-mono text-sm">{value}</dd>
    </div>
  )
}

function RequestMetricsSection({ metrics }: { metrics: Record<string, unknown> }) {
  const { t } = useI18n()
  const fields: Array<[string, string | number | boolean]> = [
    ["available", metrics.available as string | number | boolean],
    ["total_requests", metrics.total_requests as string | number | boolean],
    ["hits", metrics.hits as string | number | boolean],
    ["misses", metrics.misses as string | number | boolean],
    ["errors", metrics.errors as string | number | boolean],
    ["hit_rate", metrics.hit_rate as string | number | boolean],
    ["avg_latency_ms", metrics.avg_latency_ms as string | number | boolean],
  ].filter(([, value]) => value !== undefined && value !== null) as Array<[string, string | number | boolean]>

  if (fields.length === 0) return null

  return (
    <section className="border-b pb-4">
      <SectionHeading title={t("cacheRequestMetrics")} titleAs="h2" className="border-b-0 pb-3" />
      <dl className="grid gap-3 text-sm sm:grid-cols-2 lg:grid-cols-3">
        {fields.map(([label, value]) => (
          <ScalarField key={label} label={label} value={String(value)} />
        ))}
      </dl>
    </section>
  )
}

export function CacheOverviewContent({ tree }: { tree: CacheTree }) {
  const { t } = useI18n()

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <section className="border-b pb-4">
        <SectionHeading title={t("cacheStorageTitle")} titleAs="h2" className="border-b-0 pb-3" />
        <dl className="grid gap-3 text-sm sm:grid-cols-2 lg:grid-cols-4">
          {tree.backend ? <ScalarField label={t("backend")} value={tree.backend} /> : null}
          <ScalarField label={t("cacheNamespace")} value={tree.namespace} />
          {tree.scope ? <ScalarField label={t("scope")} value={tree.scope} /> : null}
          <ScalarField label={t("cacheCollections")} value={String(tree.totalCollections)} />
          <ScalarField label={t("cacheTotalKeys")} value={String(tree.totalKeys)} />
        </dl>
      </section>
      {tree.requestMetrics ? <RequestMetricsSection metrics={tree.requestMetrics} /> : null}
      <section className="pb-2">
        <SectionHeading title={t("cacheLayers")} titleAs="h2" className="border-b-0 pb-3" />
        <div className="grid gap-2 sm:grid-cols-2">
          {tree.layers.map((layerNode) => (
            <div key={layerNode.layer} className="rounded-md border px-3 py-3">
              <div className="flex items-center justify-between gap-3">
                <p className="font-medium">{t(LAYER_I18N_KEY[layerNode.layer])}</p>
                <Badge variant="outline">{layerNode.layer}</Badge>
              </div>
              <p className="mt-2 text-sm text-muted-foreground">
                {t("cacheLayerSummary", { types: layerNode.typeCount, keys: layerNode.keyCount })}
              </p>
            </div>
          ))}
        </div>
      </section>
    </div>
  )
}

export function CacheLayerContent({ collection }: { collection: CacheCollectionNode | null }) {
  const { t } = useI18n()

  if (!collection) {
    return <p className="text-sm text-muted-foreground">{t("cacheSelectCollectionHint")}</p>
  }

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
      <CacheCollectionDetail collection={collection} />
    </div>
  )
}
